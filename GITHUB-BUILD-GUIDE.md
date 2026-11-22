# 方案 B：让 GitHub 帮你编译 Windows 版（保姆级）

你不装 VS、不装 Rust、不装 Node，也不跑 PowerShell。把源码传到 GitHub，
用 GitHub 免费提供的 Windows 机器编译，最后下载 exe 和安装包。

全程只需要：一个 GitHub 账号 + GitHub Desktop（图形界面，不用记 git 命令）。

预计时间：上传 5 分钟，编译 40–70 分钟，下载 2 分钟。

---

## 0. 先确认你手里有这三样

1. 已解压的源码文件夹，里面能看到：
   - `package.json`
   - `src-tauri` 文件夹
   - `.github` 文件夹（点进去有 `workflows\build-obs-windows.yml`）
2. GitHub 账号（没有就现注册：https://github.com/signup）
3. GitHub Desktop（下载：https://desktop.github.com/download/）

> `.github` 是点开头的文件夹。Windows 资源管理器默认能显示；
> 如果看不到，点“查看”→“显示”→“隐藏的项目”打勾。

---

## 1. 安装并登录 GitHub Desktop

1. 双击下载的 `GitHubDesktopSetup-x64.exe`，一路 Next 安装。
2. 打开 GitHub Desktop，点 **Sign in to GitHub.com**。
3. 浏览器会打开授权页，点 **Authorize desktop**，回到 Desktop 即登录成功。
4. 如果提示配置用户名/邮箱，按它默认的填就行（只影响提交记录显示）。

---

## 2. 新建一个空仓库

1. 菜单 **File → New repository…**（中文界面：文件 → 新建仓库）。
2. 填写：
   - **Name**：`AudioScript-OBS`（只能英文数字和减号）
   - **Description**：随便写，可留空
   - **Local path**：选一个空间足够的盘，例如 `D:\GitHub`。
     最终会生成 `D:\GitHub\AudioScript-OBS`
   - **Initialize this repository with a README**：**不要勾**（如果界面有这个选项）
   - **Git ignore** / **License**：都选 **None**
3. 点 **Create repository**。

此时文件夹里只有一个隐藏的 `.git`，这是正常的。

---

## 3. 把源码复制进仓库文件夹

1. 打开你解压出来的源码文件夹。
2. `Ctrl+A` 全选，`Ctrl+C` 复制。
3. 打开 `D:\GitHub\AudioScript-OBS`，`Ctrl+V` 粘贴。
4. 如果提示同名文件，选“替换目标中的文件”。

复制完检查两个路径是否存在：

```
D:\GitHub\AudioScript-OBS\package.json
D:\GitHub\AudioScript-OBS\.github\workflows\build-obs-windows.yml
```

**最容易出错的地方**：不要把解压出来的上一层文件夹整个粘进去，
否则会变成 `AudioScript-OBS\AudioScript-0.7.2\package.json`，GitHub 就找不到工作流。

---

## 4. 提交并发布到 GitHub

1. 回到 GitHub Desktop，左侧会显示几百个 Changed files，这是正常的。
2. 左下角 **Summary** 填：`init AudioScript OBS build`
3. 点 **Commit to main**。
4. 顶部出现 **Publish repository**，点它。
5. 弹窗里：
   - **Name**：保持 `AudioScript-OBS`
   - **Keep this code private**：
     - 勾选 = 私有仓库，别人看不到；免费账号每月 2000 分钟 Actions，够用。
     - 不勾 = 公开仓库，Actions 不限时长，但源码公开。
   - 建议先勾私有；如果提示额度不够，再改公开。
6. 点 **Publish repository**，等几十秒。

浏览器打开 `https://github.com/你的用户名/AudioScript-OBS`，
能看到 `package.json` 和 `.github` 就说明上传成功。

---

## 5. 在网页上启动编译

1. 进入仓库页面，点上方的 **Actions** 标签。
2. 第一次进来如果出现绿色按钮
   **I understand my workflows, go ahead and enable them**，点它。
3. 左侧工作流列表里点 **Build OBS Windows**。
4. 右侧有 **Run workflow** 下拉按钮：
   - Branch 选 **main**
   - 点绿色 **Run workflow**
5. 刷新页面，会多出一条黄色/绿色的运行记录。

---

## 6. 等它跑完（40–70 分钟）

点进这条运行记录，左侧会依次出现这些步骤：

| 步骤 | 大约耗时 | 在干什么 |
| --- | --- | --- |
| Checkout | 10 秒 | 把仓库拉到编译机 |
| Setup pnpm / Node / Rust / Python | 1–2 分钟 | 装工具链 |
| Install dependencies | 2–5 分钟 | `pnpm install` 装前端依赖 |
| Ensure typeshare 1.13.4 | 3–10 分钟 | 首次要 `cargo install` 编译，并写回 PATH |
| Fetch ASR / VAD models | 2–5 分钟 | 下载语音识别模型到 `src-tauri/assets/models` |
| Fetch MT models (optional) | 10–25 分钟 | 装 torch 并转换翻译模型；失败只显示橙色警告，不阻断构建 |
| Build Tauri app (NSIS) | 20–40 分钟 | Rust release 编译 + 打包 |
| Upload installers | 10 秒 | 上传产物 |

看到最上面变成绿色 **Success** 就是成功；红色 **Failure** 就点开红色步骤看最后 20 行报错。

编译期间可以关掉网页，不影响。

---

## 7. 下载编译结果

1. 打开这次运行页面，滚到最底部 **Artifacts** 区域。
2. 点 **AudioScript-OBS-Windows-x64** 下载 zip（需要登录 GitHub）。
3. 解压后里面有两个文件：
   - `AudioScript.exe`（裸程序，单独拿走会缺模型，不建议直接双击）
   - `AudioScript_0.7.2_x64-setup.exe`（**用这个安装**）

双击安装包，SmartScreen 可能提示“未知发布者”（因为没买代码签名证书），
点 **更多信息 → 仍要运行** 即可。

---

## 8. 验证 17890 端口和 OBS

1. 安装完启动 AudioScript，开始采集/转写。
2. 打开 cmd 执行：

```bat
curl.exe http://127.0.0.1:17890/health
```

返回类似：

```json
{"ok":true,"name":"audioscript-obs","clients":0,"segments":0,"historyLimit":600}
```

就说明原生 OBS 端口已经在了。

3. OBS 里添加 **浏览器** 源：
   - URL：`http://127.0.0.1:17890/overlay`
   - 宽 1920、高 1080
   - 勾掉“关闭源时关闭浏览器”（可选）
4. 说两句话，字幕应实时出现。

如果 17890 没起来，脚本会自动顺延 17891–17899，
在 App 日志里搜 `OBS 浏览器源已就绪` 拿实际端口。

---

## 9. 常见问题

**Actions 里没有 Build OBS Windows**

- `.github/workflows/build-obs-windows.yml` 没上传成功，或不在 main 分支。
- 仓库 Settings → Actions → General，确认选的是 **Allow all actions and reusable workflows**。

**`ERR_PNPM_OUTDATED_LOCKFILE`**

你或编辑器改过 `package.json`。在 GitHub Desktop 里右键该文件 → Discard changes，再提交。

**`Ensure typeshare` 这一步失败**

多为 crates.io 网络抖动，点右上角 **Re-run failed jobs** 重跑即可。

**`Fetch MT models (optional)` 显示橙色警告**

它已设 `continue-on-error`，不会让整次构建失败，只是没有翻译模型。
字幕原样输出正常；想要翻译就重跑，或按下面把它删掉：
GitHub 网页打开 workflow 文件 → 铅笔 → 删除该步骤的四行 → Commit changes。

**`Build Tauri app` 报 `link.exe` / `Windows SDK` 找不到**

GitHub 的 Windows 机器偶尔镜像异常，重跑一次通常就好。

**`LNK2038 RuntimeLibrary mismatch`**

说明预编译库的 CRT 模式和 `RUSTFLAGS` 不一致。把 workflow 里 Build 步骤的

```yaml
        env:
          RUSTFLAGS: -C target-feature=+crt-static
```

两行删掉再跑；如果反过来报 `/MT` 不匹配，就再加回来。

**跑了 3 小时还没结束**

工作流已设 180 分钟超时，会自动失败。重跑；仍超时就删掉 MT 那一步再跑。

**下载按钮是灰的 / 找不到 Artifacts**

编译失败了就没有产物；先确认运行记录是绿色。

**安装包被杀毒软件拦**

没有代码签名，属于正常误报。可先校验文件大小，或把源码自己再编一次。

---

## 10. 可选：以后改代码怎么重新编

在 GitHub Desktop 里改完文件 → Summary 写一句 → **Commit to main** → **Push origin**。
然后到 Actions 页面再点一次 **Run workflow**。

想自动编：在 GitHub 网页 **Releases → Draft a new release**，
Tag 填 `v0.7.2-obs`，发布后会自动触发（工作流里配了 `v*` 标签触发）。

---

## 附：这条工作流到底做了什么

文件：`.github/workflows/build-obs-windows.yml`

1. 租一台 `windows-latest`（Windows Server 2022 + VS 2022 + Windows SDK）。
2. 装 Node 22、pnpm 10（读 `package.json` 的 `packageManager`）、Rust stable-msvc、Python 3.12。
3. `pnpm install --frozen-lockfile`，再确保 `typeshare-cli 1.13.4`。
4. 下载 ASR/MT 模型到 `src-tauri/assets/models`。
5. 设 `RUSTFLAGS=-C target-feature=+crt-static`，执行 `pnpm tauri build --bundles nsis`。
6. 把 `AudioScript.exe` 和 NSIS 安装包上传成 Artifacts。

全程只读仓库，不需要任何 Secret 或签名私钥。
