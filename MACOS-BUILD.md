# 在 MacBook（M1 Pro / Apple Silicon）上编译 AudioScript-OBS

你有两条路，选一条即可：

- **路线 A：让 GitHub 的 macOS 机器编**。本机不用装 Rust/Node/Xcode，上传源码，等 30–60 分钟，下载 DMG。
- **路线 B：在本机 Mac 上编**。需要 Xcode Command Line Tools、Homebrew、Node 22、Rust、Python 3.12，首次装环境约 15–25 分钟，编译 20–40 分钟。

两条路编出来的都是 **arm64 原生版**，自带 `127.0.0.1:17890` 的 OBS 字幕接口，不是 Intel 转译版。

> 注意：项目要求 macOS **14.2 或更高**（`tauri.conf.json` 的 `minimumSystemVersion`）。
> 先看一眼：左上角  → 关于本机，或终端 `sw_vers`。

---

## 路线 A：GitHub 编（推荐，本机不装工具链）

### A1. 准备

1. GitHub 账号：https://github.com/signup
2. GitHub Desktop：https://desktop.github.com/download/
3. 解压 `AudioScript-0.7.2-macOS-OBS-Source.zip`。

### A2. 上传源码

1. 安装并打开 GitHub Desktop → **Sign in to GitHub.com** → 浏览器 **Authorize desktop**。
2. 菜单 **File → New repository**：
   - Name：`AudioScript-OBS`
   - Local path：`~/GitHub`（路径尽量纯英文）
   - 不要勾初始化 README
   - Create repository
3. 把解压出来的**里面那一层**所有文件（`package.json`、`src-tauri`、`.github` 等）
   全选复制到 `~/GitHub/AudioScript-OBS`。
4. 回 GitHub Desktop，Summary 填 `init AudioScript OBS macOS` → **Commit to main** →
   **Publish repository**。
5. 仓库类型：
   - **公开**：Actions 不限时长，最省事；
   - **私有**：可以，但 macOS 机器按 **10 倍** 计费（跑 1 小时算 10 小时额度），
     免费额度通常够一两次，长期反复编建议公开。

### A3. 跑工作流

1. 仓库网页 → **Actions**。
2. 第一次若提示，点 **I understand my workflows, go ahead and enable them**。
3. 左侧选 **Build OBS macOS (Apple Silicon)**。
4. 右侧 **Run workflow** → Branch 选 **main** → 绿色 **Run workflow**。
5. 点进运行记录看步骤，正常 30–60 分钟。

关键步骤：

| 步骤 | 耗时 | 说明 |
| --- | --- | --- |
| Assert arm64 runner | 1 秒 | 确认不是 Intel 机器，否则直接报错 |
| Setup … | 2 分钟 | 装 Node 22、pnpm 10、Rust、Python 3.12 |
| Ensure typeshare 1.13.4 | 3–10 分钟 | 首次编译 typeshare |
| Fetch ASR / VAD models | 2–5 分钟 | 下载识别模型 |
| Fetch MT models (optional) | 10–25 分钟 | 翻译模型；失败只橙色警告，不阻断 |
| Build Tauri app (DMG) | 20–40 分钟 | Rust release 编译 + 打 DMG |

### A4. 下载与安装

运行页面最底部 **Artifacts** → **AudioScript-OBS-macOS-ARM**，下载 zip。
里面：

- `AudioScript_0.7.2_aarch64.dmg`
- `AudioScript.app.zip`（不想装 DMG 可直接拖出来用）

打开 DMG，把 **AudioScript** 拖进“应用程序”。

因为没买 Apple 开发者签名，第一次打开会被 Gatekeeper 拦。两种解法：

- 右键 App → **打开** → 再点 **打开**；
- 或终端执行：

```bash
xattr -dr com.apple.quarantine /Applications/AudioScript.app
open /Applications/AudioScript.app
```

---

## 路线 B：本机 Mac 编译

### B1. 装 Xcode Command Line Tools

终端执行：

```bash
xcode-select --install
```

弹窗点“安装”，同意协议，等 5–10 分钟。装完验证：

```bash
xcode-select -p
xcrun --show-sdk-path
swift --version
```

> `translation-rs` 会用 `swift build` 编译一个 Swift 桥接库，所以必须有 Swift 工具链。
> 如果后面报 Swift/Xcode 相关错误，去 App Store 装完整 **Xcode**，再执行：
> `sudo xcode-select -s /Applications/Xcode.app/Contents/Developer`

### B2. 装 Homebrew

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

装完按提示把 brew 加进 PATH（Apple Silicon 默认 `/opt/homebrew`）：

```bash
echo 'eval "$(/opt/homebrew/bin/brew shellenv)"' >> ~/.zprofile
eval "$(/opt/homebrew/bin/brew shellenv)"
```

### B3. 装 Node 22 + pnpm

```bash
brew install node@22
export PATH="$(brew --prefix node@22)/bin:$PATH"
echo 'export PATH="$(brew --prefix node@22)/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
node -v          # 应为 v22.x
npm install -g pnpm@10.29.2
pnpm -v          # 应为 10.29.2
```

`node@22` 是 keg-only，不加 PATH 的话系统里仍然找不到 `node`，这是最容易卡住的一步。

### B4. 装 Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup default stable
rustup target add aarch64-apple-darwin
rustc -vV        # host 应为 aarch64-apple-darwin
```

### B5. 装 Python 3.12（翻译模型需要）

```bash
brew install python@3.12
export PATH="$(brew --prefix python@3.12)/libexec/bin:$PATH"
echo 'export PATH="$(brew --prefix python@3.12)/libexec/bin:$PATH"' >> ~/.zshrc
python3 -V       # 必须是 3.12.x，不能是系统自带的 3.9
```

不装也能编，只是没有翻译模型；`pnpm fetch:mt` 会失败，字幕原样输出仍可用。

### B6. 编译

进入源码目录（路径换成你自己的）：

```bash
cd ~/Downloads/AudioScript-0.7.2-macOS-OBS-Source
pnpm install --frozen-lockfile
cargo install typeshare-cli --version 1.13.4 --locked
pnpm fetch:model
pnpm fetch:mt          # 可选；跳过就不带翻译模型
pnpm tauri build --bundles dmg
```

或者直接双击/执行我放的脚本：

```bash
chmod +x BUILD-MACOS.command
bash BUILD-MACOS.command
```

**不要在 macOS 上设 `RUSTFLAGS=-C target-feature=+crt-static`**，那是 Windows 专用的；
macOS 用了反而会链接失败。

产物：

```
src-tauri/target/release/bundle/macos/AudioScript.app
src-tauri/target/release/bundle/dmg/AudioScript_0.7.2_aarch64.dmg
```

本机编出来的 App 没有 quarantine 标记，直接双击即可。

---

## 验证 OBS 接口

启动 App，开始转写，然后终端：

```bash
curl http://127.0.0.1:17890/health
```

返回 `"ok":true` 即成功。OBS 添加**浏览器源**：

```
http://127.0.0.1:17890/overlay
```

宽 1920、高 1080。首次开 App 会请求**麦克风**和**屏幕录制**权限，
在“系统设置 → 隐私与安全性”里给 AudioScript 打勾，否则采集不到声音。

---

## 常见错误

| 现象 | 原因 / 处理 |
| --- | --- |
| `xcrun: error: invalid active developer path` | CLT 没装好：`xcode-select --install`；或 `sudo xcode-select --reset` |
| `swift build failed` | 只有 CLT 不够时，装完整 Xcode 后 `sudo xcode-select -s /Applications/Xcode.app/Contents/Developer` |
| `node: command not found` | `node@22` 没加 PATH，见 B3 |
| `python3` 是 3.9，pip 报 `No matching distribution` | PATH 没指向 3.12，见 B5 |
| `LNK2038` / `crt-static` | 这是 Windows 的报错；macOS 不要设 RUSTFLAGS |
| `Package requires macOS 14.2` | 系统太旧；升级 macOS，或改 `tauri.conf.json` 的 `minimumSystemVersion` 后重编 |
| App 打开提示“已损坏” | `xattr -dr com.apple.quarantine /Applications/AudioScript.app` |
| 17890 没监听 | 端口被占会顺延 17891–17899，日志搜“OBS 浏览器源已就绪” |

---

## 工作流里做了什么

`.github/workflows/build-obs-macos.yml`：

1. 用 `macos-14`（Apple Silicon）runner，并断言 `uname -m = arm64`。
2. 装 Node 22、pnpm 10、Python 3.12、Rust `aarch64-apple-darwin`。
3. `pnpm install --frozen-lockfile`，确保 typeshare 1.13.4。
4. 下载 ASR/VAD 模型；翻译模型设为可选（`continue-on-error`）。
5. `pnpm tauri build --bundles dmg`，**不带** `tauri.updater.conf.json`，所以不需要签名私钥。
6. 上传 DMG 和 `AudioScript.app.zip` 两个产物。

全程只读仓库，不需要任何 Secret。
