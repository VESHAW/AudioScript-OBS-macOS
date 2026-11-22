# 构建带原生 17890 端口的 Windows 版

这个源码树已经打好了 OBS 补丁：`src-tauri/src/obs/mod.rs` 会在 App 内起一个
只监听 `127.0.0.1` 的 HTTP + SSE 服务，默认端口 **17890**。编译出来后**不需要**
调试端口、不需要 `win_bridge.py`，OBS 直接连 `http://127.0.0.1:17890/overlay`。

构建有两种方式：本机 Windows 编译，或让 GitHub Actions 免费帮你编。

---

## 1. 本机 Windows 构建（推荐先试）

### 第 0 步：一键装环境（推荐）

解压源码包后，**右键 `setup-build-env.bat` → “以管理员身份运行”**。它是纯批处理，
不受 PowerShell 脚本策略影响，只会弹一次 UAC。

它用 winget 依次安装并校验：

| 组件 | 装什么 | 作用 |
| --- | --- | --- |
| VS 2022 Build Tools | `VCTools` + 推荐组件 + CMake（含 Windows SDK） | MSVC 链接器、Windows SDK、CMake |
| Rust | Rustup + `stable-msvc` + rustfmt | 编译 `src-tauri` |
| Node.js | `OpenJS.NodeJS.22`（22.x） | 与官方 CI 一致，跑 Vite / pnpm |
| pnpm | `10.29.2` | 与 `packageManager` 字段一致 |
| Python | `3.12`（可选） | 官方 CI 同版本，部分脚本会用 |

时间约 15–40 分钟，磁盘预留 8–12 GB。装完会打印 `node -v / pnpm -v / cargo -V /
rustc host` 自检结果，并问你要不要直接开始 `BUILD-WINDOWS.bat`。

> Win10/11 自带 WebView2 Runtime；精简版系统才需要另装
> [WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)。

**找不到 winget 怎么办？**

脚本已经内置兜底：先在 PATH、`%LocalAppData%\Microsoft\WindowsApps`、`%SystemRoot%\System32`
三处找 `winget.exe`；仍然找不到就自动改走官方安装包直装，不再要求 Microsoft Store：

| 组件 | 直装方式 |
| --- | --- |
| VS Build Tools | `https://aka.ms/vs/17/release/vs_BuildTools.exe` + 同样的 `--add` 参数 |
| Rust | `rustup-init.exe`（`x86_64-pc-windows-msvc`）+ `-y --default-toolchain stable-msvc` |
| Node 22 | 从 `nodejs.org/dist/latest-v22.x/` 解析 `win-x64.msi` 后 `msiexec /qn` |
| pnpm | `npm i -g pnpm@10.29.2` |
| Python | 直装模式跳过（可选，不影响编译） |

下载器优先用系统自带的 `curl.exe`，没有就退回 `certutil -urlcache`，两条都不依赖 PowerShell。

**仍想恢复 winget 的话：**

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override `
  "--wait --quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --add Microsoft.VisualStudio.Component.VC.CMake.Project"
winget install Rustlang.Rustup -e
rustup default stable-msvc
winget install OpenJS.NodeJS.22 -e
npm i -g pnpm@10.29.2
```

验证：

```powershell
node -v          # v22.x
pnpm -v          # 10.x
rustc -vV        # host: x86_64-pc-windows-msvc
```

### 然后双击 `BUILD-WINDOWS.bat`

它会按顺序执行（也可以手动一条条跑）：

```bat
set RUSTFLAGS=-C target-feature=+crt-static
pnpm install --frozen-lockfile
cargo install typeshare-cli --version 1.13.4 --locked
pnpm fetch:mt
pnpm tauri build --bundles nsis
```

产物：

```
src-tauri\target\release\AudioScript.exe
src-tauri\target\release\bundle\nsis\AudioScript_0.7.2_x64-setup.exe
```

安装后启动 App，日志里应出现（`%APPDATA%\com.audioscript.desktop\logs\` 或控制台）：

```
OBS 桥接已启动：http://127.0.0.1:17890/overlay
```

验证端口：

```powershell
curl.exe http://127.0.0.1:17890/health
# {"ok":true,"name":"audioscript-obs-bridge","clients":0,"segments":0,...}
```

OBS → 来源 → 浏览器 → `http://127.0.0.1:17890/overlay?lines=3&fontSize=44`。
历史条数默认 600，要整场不封顶：启动前 `$env:AUDIOSCRIPT_OBS_HISTORY="0"`。

> 说明：构建脚本**没有**用 `src-tauri/tauri.updater.conf.json`，因此不需要
> `TAURI_SIGNING_PRIVATE_KEY`；代价是安装包不带自动更新签名。要发版的话，
> 按官方 CI 的方式配好签名密钥再 `--config src-tauri/tauri.updater.conf.json`。

---

## 2. 让 GitHub Actions 编（本机不用装 VS）

源码里已经放好 `.github/workflows/build-obs-windows.yml`，触发方式是手动。

**第一次操作、不想碰 git 命令**：直接看根目录
[`GITHUB-BUILD-GUIDE.md`](GITHUB-BUILD-GUIDE.md)，用 GitHub Desktop 点鼠标完成
上传 → 运行 → 下载，每一步都有截图级描述。

已经有 git 环境的话：

```bash
git init
git add .
git commit -m "AudioScript 0.7.2 + OBS bridge"
git branch -M main
git remote add origin https://github.com/<你的账号>/<仓库名>.git
git push -u origin main
```

然后打开仓库 → **Actions** → 左侧 **Build OBS Windows** → **Run workflow**。
约 40–70 分钟后，在该次运行底部下载 artifact
`AudioScript-OBS-Windows-x64`（含 `AudioScript.exe` 和 NSIS 安装包）。

工作流用 `windows-latest` + Node 22 + pnpm 10 + stable-msvc + 静态 CRT，
和本机脚本走同一条命令链；`RUSTFLAGS` 只加在 Tauri 构建那一步，
避免 `cargo install typeshare` 的 proc-macro 也被 `/MT` 影响。

## 3. 为什么不能在这台 Linux 机器上直接给你 exe

Tauri 的 Windows 打包依赖 MSVC 链接器、Windows SDK、WebView2 导入库，以及
`sherpa-onnx` / `ct2rs` 的 Windows 预编译静态库（`/MT`）；这些在 Linux 上没有可用的
交叉工具链，官方也不支持从 Linux 交叉打 Windows 安装包。所以这里交付的是
**已打补丁、依赖已锁定、一条命令可复现**的源码树，而不是伪装成能跑的二进制。

---

## 4. 常见失败

| 报错 | 处理 |
| --- | --- |
| `LNK2038 ... RuntimeLibrary mismatch` | 忘了 `RUSTFLAGS=-C target-feature=+crt-static`；在同一个 cmd 窗口里设置后再 build |
| `link.exe not found` / `cargo build` 找不到 linker | VS Build Tools 没勾 C++ 工作负载，或没装 Windows SDK |
| `failed to run custom build command for sherpa-onnx-sys` | 删 `src-tauri\target\sherpa-onnx-prebuilt` 重试；确保网络能下预编译包 |
| `typeshare` 版本不对 | `cargo install typeshare-cli --version 1.13.4 --locked` |
| `pnpm fetch:mt` 卡住 | 模型在 HuggingFace，配代理或换网络；失败可重跑，已下载的会跳过 |
| NSIS 下载失败 | 重跑；或 `--bundles msi` 改用 WiX |
| 启动后 17890 没监听 | 看日志里 `OBS 桥接` 的 warn；端口被占会顺延到 17899（日志会给实际地址） |
| 端口顺延了但 OBS 还连 17890 | 设 `AUDIOSCRIPT_OBS_PORT` 固定起始端口，或按日志改 OBS 地址 |

补丁回滚：源码里已删除 `*.bak.obs`；要回到原版就重新解压官方 0.7.2 源码，
不要再跑 `apply_patch.py`。
