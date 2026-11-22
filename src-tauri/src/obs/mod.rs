//! OBS 浏览器源桥接（本地 HTTP + SSE）。
//!
//! 只做三件事：
//! 1. 把 `transcript://segments` 的最新快照留在内存里（按 id 合并，默认最多 [`DEFAULT_HISTORY`] 条（`AUDIOSCRIPT_OBS_HISTORY` 可调，`0` 不封顶））；
//! 2. 用 Server-Sent Events 把增量推给订阅者（OBS 浏览器源 / 任何本地网页）；
//! 3. 把内置的单文件字幕页当作 OBS「浏览器源」直接提供，免装任何插件。
//!
//! 只监听 `127.0.0.1`，不开公网端口。端口默认 17890，可用环境变量 `AUDIOSCRIPT_OBS_PORT` 覆盖。
//!
//! 本模块不依赖 tauri / tokio，只用 std + serde_json，方便单独编译与测试。

use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::json;

use crate::transcript::TranscriptSegment;

/// OBS 浏览器源的默认端口。
pub const DEFAULT_PORT: u16 = 17890;

/// 端口被占用时向后顺延的次数（17890 → 17899）。
const PORT_SCAN: u16 = 10;
/// 默认保留的历史条数。OBS 端只显示最后几行，但 `/state`、`/transcript.json`
/// 需要能给出整场字幕，所以这里留得比页面显示多得多（600 条 ≈ 一小时连续说话）。
const DEFAULT_HISTORY: usize = 600;

/// 历史条数上限：`AUDIOSCRIPT_OBS_HISTORY` 可覆盖，`0` 表示本场不封顶。
fn history_limit() -> usize {
    static LIMIT: OnceLock<usize> = OnceLock::new();
    *LIMIT.get_or_init(|| match std::env::var("AUDIOSCRIPT_OBS_HISTORY") {
        Ok(raw) => match raw.trim().parse::<usize>() {
            Ok(0) => 0,
            Ok(n) => n,
            Err(_) => DEFAULT_HISTORY,
        },
        Err(_) => DEFAULT_HISTORY,
    })
}
/// 同时订阅的客户端上限，防止被无限开标签页拖垮。
const MAX_CLIENTS: usize = 16;
/// SSE 心跳间隔；超过它没有数据就发一行注释保活，避免中间设备掐断空闲连接。
const KEEPALIVE: Duration = Duration::from_secs(15);
/// 内置字幕页（编译期打进二进制，发布后不需要额外文件）。
const PAGE: &str = include_str!("../../assets/obs-caption.html");

struct Client {
    id: u64,
    tx: Sender<String>,
}

struct Hub {
    clients: Mutex<Vec<Client>>,
    retained: Mutex<Vec<TranscriptSegment>>,
    languages: Mutex<(String, String)>,
    running: AtomicBool,
    next_id: AtomicU64,
}

static HUB: OnceLock<Hub> = OnceLock::new();

static PORT: AtomicU16 = AtomicU16::new(0);
static START_LOCK: Mutex<()> = Mutex::new(());

fn hub() -> &'static Hub {
    HUB.get_or_init(|| Hub {
        clients: Mutex::new(Vec::new()),
        retained: Mutex::new(Vec::new()),
        languages: Mutex::new(("en".into(), "zh".into())),
        running: AtomicBool::new(false),
        next_id: AtomicU64::new(1),
    })
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 浏览器源地址（带端口）。
pub fn url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/overlay")
}

/// 当前监听端口；未启动时为 0。
pub fn port() -> u16 {
    PORT.load(Ordering::SeqCst)
}

/// 启动桥接服务，返回可直接粘进 OBS 的地址。重复调用幂等。
pub fn start() -> Result<String, String> {
    let _guard = lock(&START_LOCK);
    let running = PORT.load(Ordering::SeqCst);
    if running != 0 {
        return Ok(url(running));
    }

    let listener = bind_listener()?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("OBS 桥接取本地地址失败：{e}"))?
        .port();

    PORT.store(port, Ordering::SeqCst);
    let spawned = std::thread::Builder::new()
        .name("obs-sse-accept".into())
        .spawn(move || accept_loop(listener));

    if let Err(e) = spawned {
        PORT.store(0, Ordering::SeqCst);
        return Err(format!("启动 OBS 桥接线程失败：{e}"));
    }

    tracing::info!(port, "OBS 桥接已启动：{}", url(port));
    Ok(url(port))
}

fn bind_listener() -> Result<TcpListener, String> {
    let base = std::env::var("AUDIOSCRIPT_OBS_PORT")
        .ok()
        .and_then(|v| v.trim().parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT);

    let mut last = String::from("未尝试绑定");
    for offset in 0..PORT_SCAN {
        let Some(port) = base.checked_add(offset) else {
            break;
        };
        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(listener) => return Ok(listener),
            Err(e) => last = format!("127.0.0.1:{port}：{e}"),
        }
    }

    Err(format!(
        "OBS 桥接没有可用端口（{last}）；可用环境变量 AUDIOSCRIPT_OBS_PORT 指定起始端口"
    ))
}

fn accept_loop(listener: TcpListener) {
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let spawned = std::thread::Builder::new()
                    .name("obs-sse-conn".into())
                    .spawn(move || handle_conn(stream));
                if let Err(e) = spawned {
                    tracing::warn!("OBS 桥接连接线程启动失败：{e}");
                }
            }
            Err(e) => {
                tracing::warn!("OBS 桥接接受连接失败：{e}");
                std::thread::sleep(Duration::from_millis(200));
            }
        }
    }
    tracing::debug!("OBS 桥接监听线程退出");
}

fn handle_conn(mut stream: TcpStream) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    let Some((method, path)) = read_request(&stream) else {
        return;
    };
    let route = route_of(&path);

    match (method.as_str(), route.as_str()) {
        ("OPTIONS", _) => {
            let _ = write_empty(&mut stream, "204 No Content");
        }
        ("GET", "/") | ("GET", "/overlay") | ("GET", "/index.html") => {
            let _ = write_response(
                &mut stream,
                "200 OK",
                "text/html; charset=utf-8",
                PAGE.as_bytes(),
            );
        }
        ("GET", "/events") => stream_events(&mut stream),
        ("GET", "/state") => {
            let body = state_json(true).to_string();
            let _ = write_response(
                &mut stream,
                "200 OK",
                "application/json; charset=utf-8",
                body.as_bytes(),
            );
        }
        // 整场字幕（受 AUDIOSCRIPT_OBS_HISTORY 限制），给外部脚本 / 二次开发用。
        ("GET", "/transcript.json") => {
            let body =
                serde_json::to_string(&*lock(&hub().retained)).unwrap_or_else(|_| "[]".to_string());
            let _ = write_response(
                &mut stream,
                "200 OK",
                "application/json; charset=utf-8",
                body.as_bytes(),
            );
        }
        ("GET", "/transcript.md") => {
            let body = transcript_markdown(&lock(&hub().retained));
            let _ = write_response(
                &mut stream,
                "200 OK",
                "text/markdown; charset=utf-8",
                body.as_bytes(),
            );
        }
        ("GET", "/health") => {
            let body = json!({
                "ok": true,
                "name": "audioscript-obs-bridge",
                "clients": client_count(),
                "segments": retained_count(),
                "historyLimit": history_limit(),
            })
            .to_string();
            let _ = write_response(
                &mut stream,
                "200 OK",
                "application/json; charset=utf-8",
                body.as_bytes(),
            );
        }
        ("GET", "/favicon.ico") => {
            let _ = write_empty(&mut stream, "204 No Content");
        }
        _ => {
            let _ = write_response(
                &mut stream,
                "404 Not Found",
                "text/plain; charset=utf-8",
                b"not found",
            );
        }
    }
}

/// 取出请求行里的方法 + 路径，读完请求头即返回（不读 body，本服务只有 GET）。
fn read_request(stream: &TcpStream) -> Option<(String, String)> {
    let clone = stream.try_clone().ok()?;
    let mut reader = BufReader::new(clone);

    let mut line = String::new();
    if reader.read_line(&mut line).ok()? == 0 || line.len() > 4096 {
        return None;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_ascii_uppercase();
    let path = parts.next()?.to_string();

    let mut total = line.len();
    loop {
        let mut header = String::new();
        let n = reader.read_line(&mut header).ok()?;
        if n == 0 || header.trim_end().is_empty() {
            break;
        }
        total += n;
        if total > 16 * 1024 {
            return None;
        }
    }

    Some((method, path))
}

fn route_of(path: &str) -> String {
    path.split('?').next().unwrap_or("/").to_string()
}

fn write_empty(stream: &mut TcpStream, status: &str) -> io::Result<()> {
    let head = format!(
        "HTTP/1.1 {status}\r\nAccess-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Methods: GET, HEAD, OPTIONS\r\n\
         Access-Control-Allow-Headers: *\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(head.as_bytes())?;
    stream.flush()
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) -> io::Result<()> {
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Methods: GET, HEAD, OPTIONS\r\n\
         Access-Control-Allow-Headers: *\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn stream_events(stream: &mut TcpStream) {
    let head = "HTTP/1.1 200 OK\r\n\
                Content-Type: text/event-stream; charset=utf-8\r\n\
                Cache-Control: no-cache, no-store, must-revalidate\r\n\
                Pragma: no-cache\r\n\
                Access-Control-Allow-Origin: *\r\n\
                Connection: keep-alive\r\n\
                X-Accel-Buffering: no\r\n\r\n";
    if stream
        .write_all(head.as_bytes())
        .and_then(|_| stream.flush())
        .is_err()
    {
        return;
    }

    let (tx, rx) = channel::<String>();
    let id = hub().next_id.fetch_add(1, Ordering::Relaxed);
    {
        let mut clients = lock(&hub().clients);
        if clients.len() >= MAX_CLIENTS {
            let _ = stream.write_all(b"event: error\ndata: {\"reason\":\"too-many-clients\"}\n\n");
            tracing::warn!("OBS 订阅者已达上限 {MAX_CLIENTS}，拒绝新连接");
            return;
        }
        clients.push(Client { id, tx });
    }
    tracing::debug!(id, "OBS 字幕页已连接");

    let hello = hello_frame();
    if stream
        .write_all(hello.as_bytes())
        .and_then(|_| stream.flush())
        .is_err()
    {
        detach(id);
        return;
    }

    loop {
        match rx.recv_timeout(KEEPALIVE) {
            Ok(frame) => {
                if stream
                    .write_all(frame.as_bytes())
                    .and_then(|_| stream.flush())
                    .is_err()
                {
                    break;
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if stream
                    .write_all(b": ping\n\n")
                    .and_then(|_| stream.flush())
                    .is_err()
                {
                    break;
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    detach(id);
    tracing::debug!(id, "OBS 字幕页已断开");
}

fn detach(id: u64) {
    lock(&hub().clients).retain(|c| c.id != id);
}

fn client_count() -> usize {
    lock(&hub().clients).len()
}

fn retained_count() -> usize {
    lock(&hub().retained).len()
}

fn hello_frame() -> String {
    let mut out = String::from("retry: 1200\n\n");
    out.push_str(&state_frame());
    if retained_count() == 0 {
        out.push_str("event: reset\ndata: {}\n\n");
    } else {
        out.push_str(&segments_frame(&lock(&hub().retained)));
    }
    out
}

fn state_frame() -> String {
    format!("event: state\ndata: {}\n\n", state_json(false))
}

fn state_json(with_segments: bool) -> serde_json::Value {
    let (source, target) = {
        let langs = lock(&hub().languages);
        (langs.0.clone(), langs.1.clone())
    };
    let mut value = json!({
        "running": hub().running.load(Ordering::SeqCst),
        "sourceLang": source,
        "targetLang": target,
        "now": now_ms(),
    });
    if with_segments {
        let retained = lock(&hub().retained);
        value["segments"] = serde_json::to_value(&*retained).unwrap_or(json!([]));
    }
    value
}

fn segments_frame(segments: &[TranscriptSegment]) -> String {
    let data = serde_json::to_string(segments).unwrap_or_else(|_| "[]".into());
    // serde_json 会把正文里的换行转义成 \n，SSE 的 data 行不会被截断。
    format!("event: segments\ndata: {data}\n\n")
}

/// `mm:ss` / `h:mm:ss`，与 App 自带导出的时间轴写法一致。
fn clock(ms: u64) -> String {
    let total = ms / 1000;
    let (h, m, s) = (total / 3600, (total / 60) % 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

/// 整场字幕导出：只收 final / edited（与 App 的导出规则一致），带译文。
fn transcript_markdown(segments: &[TranscriptSegment]) -> String {
    let settled: Vec<&TranscriptSegment> = segments
        .iter()
        .filter(|s| s.state.is_frozen() && (s.has_text() || s.translation.is_some()))
        .collect();

    let mut out = if settled.is_empty() {
        "# AudioScript 转写\n\n_暂无已定稿字幕_\n".to_string()
    } else {
        format!("# AudioScript 转写（{} 条）\n\n", settled.len())
    };
    for seg in settled {
        let text = seg.text.trim();
        if !text.is_empty() {
            out.push_str(&format!("**[{}]** {text}\n\n", clock(seg.start_ms)));
        }
        if let Some(t) = seg
            .translation
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            out.push_str(&format!("> {t}\n\n"));
        }
    }
    out
}

impl Hub {
    fn broadcast(&self, frame: &str) {
        let mut clients = lock(&self.clients);
        let mut failed = Vec::new();
        for client in clients.iter() {
            if client.tx.send(frame.to_string()).is_err() {
                failed.push(client.id);
            }
        }
        if !failed.is_empty() {
            clients.retain(|c| !failed.contains(&c.id));
        }
    }
}

/// 推入增量段落（与 `transcript://segments` 的载荷一致），按 id 合并后广播。
pub fn publish(delta: &[TranscriptSegment]) {
    if delta.is_empty() {
        return;
    }

    let accepted = {
        let mut retained = lock(&hub().retained);
        merge(&mut retained, delta, history_limit())
    };

    if accepted.is_empty() {
        return;
    }
    hub().broadcast(&segments_frame(&accepted));
}

/// 按 id 合并增量：新 id 追加，旧 id 只在 revision 更大时覆盖；随后裁到 `limit`
/// （`limit == 0` 表示不裁）。
fn merge(
    retained: &mut Vec<TranscriptSegment>,
    delta: &[TranscriptSegment],
    limit: usize,
) -> Vec<TranscriptSegment> {
    let mut accepted: Vec<TranscriptSegment> = Vec::new();

    for seg in delta {
        if !seg.has_text() && seg.translation.is_none() {
            continue;
        }
        match retained.iter_mut().find(|r| r.id == seg.id) {
            Some(slot) => {
                if seg.revision <= slot.revision {
                    continue;
                }
                *slot = seg.clone();
                accepted.push(seg.clone());
            }
            None => {
                retained.push(seg.clone());
                accepted.push(seg.clone());
            }
        }
    }

    if limit > 0 && retained.len() > limit {
        let overflow = retained.len() - limit;
        retained.drain(0..overflow);
    }

    accepted
}

/// 清空已保留的字幕并通知所有订阅者（对应应用内「清空」）。
pub fn reset() {
    lock(&hub().retained).clear();
    hub().broadcast("event: reset\ndata: {}\n\n");
}

/// 采集状态变化（开始 / 停止 / 出错）时广播给字幕页。
pub fn set_running(running: bool) {
    if hub().running.swap(running, Ordering::SeqCst) == running {
        return;
    }
    hub().broadcast(&state_frame());
}

/// 记录本次会话翻译方向，供字幕页决定是否显示原文。
pub fn set_languages(source: &str, target: &str) {
    {
        let mut langs = lock(&hub().languages);
        if langs.0 == source && langs.1 == target {
            return;
        }
        langs.0 = source.to_string();
        langs.1 = target.to_string();
    }
    hub().broadcast(&state_frame());
}

/// 状态查询（给应用内界面显示端口、订阅人数用）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub port: u16,
    pub url: String,
    pub running: bool,
    pub clients: usize,
    pub segments: usize,
}

pub fn status() -> Status {
    let port = self::port();
    Status {
        port,
        url: if port == 0 { String::new() } else { url(port) },
        running: hub().running.load(Ordering::SeqCst),
        clients: client_count(),
        segments: retained_count(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::{SegmentState, TranslationState};

    fn seg(id: &str, text: &str, rev: u32) -> TranscriptSegment {
        TranscriptSegment {
            id: id.into(),
            start_ms: 0,
            end_ms: 100,
            text: text.into(),
            translation: None,
            translation_state: TranslationState::Idle,
            state: SegmentState::Partial,
            revision: rev,
        }
    }

    #[test]
    fn route_strips_query() {
        assert_eq!(route_of("/overlay?lines=4"), "/overlay");
        assert_eq!(route_of("/events"), "/events");
    }

    #[test]
    fn segment_frame_never_contains_bare_newline_in_data() {
        let s = seg("a", "第一行\n第二行", 1);
        let frame = segments_frame(&[s]);
        let data_line = frame
            .lines()
            .find(|l| l.starts_with("data: "))
            .expect("必须有 data 行");
        assert!(!data_line.contains('\n'), "data 行里不能出现裸换行");
        assert!(frame.ends_with("\n\n"), "SSE 帧必须以空行结束");
    }

    #[test]
    fn merge_keeps_newest_revision_and_trims() {
        let mut retained = Vec::new();

        merge(&mut retained, &[seg("a", "一", 1)], 0);
        merge(&mut retained, &[seg("a", "二", 2)], 0);
        merge(&mut retained, &[seg("a", "旧", 1)], 0);

        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0].text, "二", "低 revision 必须被忽略");

        for i in 0..(3 + 5) {
            merge(&mut retained, &[seg(&format!("s{i}"), "x", 1)], 3);
        }
        assert_eq!(retained.len(), 3, "保留条数必须封顶");
        assert_eq!(retained.last().map(|s| s.id.as_str()), Some("s7"));
    }

    #[test]
    fn limit_zero_keeps_everything() {
        let mut retained = Vec::new();
        for i in 0..50 {
            merge(&mut retained, &[seg(&format!("s{i}"), "x", 1)], 0);
        }
        assert_eq!(retained.len(), 50, "limit=0 表示不封顶");
    }

    #[test]
    fn markdown_keeps_only_settled_segments() {
        let mut a = seg("a", " 你好 ", 3);
        a.state = SegmentState::Final;
        a.translation = Some("hello".into());
        let mut b = seg("b", "进行中", 1);
        b.start_ms = 65_000;
        let md = transcript_markdown(&[a, b]);
        assert!(md.contains("**[00:00]** 你好"), "{md}");
        assert!(md.contains("> hello"), "{md}");
        assert!(!md.contains("进行中"), "partial 不应进入导出");
        assert_eq!(clock(3_725_000), "1:02:05");
        assert!(transcript_markdown(&[]).contains("暂无已定稿字幕"));
    }

    #[test]
    fn empty_text_without_translation_is_ignored() {
        let mut retained = Vec::new();
        merge(&mut retained, &[seg("a", "   ", 1)], 0);
        assert!(retained.is_empty());
    }
}
