# 这份源码改了什么（OBS 原生端口补丁）

只动了 5 个文件，不碰采集 / ASR / 翻译链路。补丁版基于官方 **0.7.2**。

| 文件 | 改动 |
| --- | --- |
| `src-tauri/src/obs/mod.rs`（新增） | 本地 HTTP + SSE 服务：只绑 `127.0.0.1`，默认 17890，被占顺延到 17899；路由 `/overlay`、`/events`、`/state`、`/transcript.json`、`/transcript.md`、`/health`；内存保留默认 600 条（`AUDIOSCRIPT_OBS_HISTORY=0` 不封顶）；最多 16 个订阅者；15 秒心跳 |
| `src-tauri/assets/obs-caption.html`（新增） | 单文件 OBS 字幕页，编译期 `include_str!` 进二进制；支持双语、partial 光标、字号/颜色/对齐/毛玻璃、断线重连、URL 参数 |
| `src-tauri/src/lib.rs` | `pub mod obs;`；`setup` 里 `session::init_emitter` 之后调用 `obs::start()`，失败只 warn 不阻断启动 |
| `src-tauri/src/session.rs` | `emit_segments` 首行 `obs::publish()`；`clear()` 末尾 `obs::reset()`；`emit_status` 里同步运行状态；`start()` 里同步翻译方向 |
| `src-tauri/src/commands/mod.rs` | `dispatch` 增加 `obs_status` 查询（返回端口、地址、客户端数、条数） |

## 怎么确认补丁真的在

```powershell
Select-String -Path src-tauri\src\lib.rs -Pattern "pub mod obs|obs::start"
Select-String -Path src-tauri\src\session.rs -Pattern "obs::publish|obs::reset|obs::set_running|obs::set_languages"
Select-String -Path src-tauri\src\commands\mod.rs -Pattern "obs_status"
```

编译后启动 App，日志应出现：

```
OBS 浏览器源已就绪：http://127.0.0.1:17890/overlay
```

`curl.exe http://127.0.0.1:17890/health` 返回 `{"ok":true,...}` 即成功。

## 没有改的东西

- 没有新增 Rust 依赖（只用 std + 已有的 serde_json / tracing）；
- 没有改 `Cargo.toml`、`pnpm-lock.yaml`、模型与 ASR 逻辑；
- 没有引入任何云端请求；
- 版本号仍是 0.7.2，bundle identifier 不变，可与官方版共存安装（但会覆盖同名应用）。
