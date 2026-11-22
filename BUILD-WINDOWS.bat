@echo off
chcp 65001 >nul
setlocal EnableExtensions
cd /d "%~dp0"

echo ============================================================
echo  AudioScript 0.7.2 + OBS 原生端口（17890）
echo  本机构建脚本：RUSTFLAGS 静态 CRT + NSIS 安装包
echo ============================================================
echo.

where node >nul 2>&1 || goto need_node
where pnpm >nul 2>&1 || goto need_pnpm
where cargo >nul 2>&1 || goto need_rust

for /f "tokens=2" %%h in ('rustc -vV ^| findstr /b "host:"') do set "HOST=%%h"
echo [i] Rust host: %HOST%
if not "%HOST%"=="x86_64-pc-windows-msvc" (
  echo [!] 当前不是 MSVC 工具链，切换到 stable-msvc...
  rustup default stable-msvc
  if errorlevel 1 goto need_vs
)

set "RUSTFLAGS=-C target-feature=+crt-static"
echo [i] RUSTFLAGS=%RUSTFLAGS%
echo.

if not exist node_modules (
  echo [1/4] pnpm install（首次会顺带下载 ASR 模型，慢是正常的）...
  call pnpm install --frozen-lockfile
  if errorlevel 1 goto fail
) else (
  echo [1/4] node_modules 已存在，跳过安装
)

echo.
echo [2/4] 确认 typeshare 1.13.4...
typeshare --version 2>nul | findstr /c:"1.13.4" >nul
if errorlevel 1 (
  cargo install typeshare-cli --version 1.13.4 --locked
  if errorlevel 1 goto fail
)

echo.
echo [3/4] 拉取翻译模型（pnpm fetch:mt）...
call pnpm fetch:mt
if errorlevel 1 goto fail

echo.
echo [4/4] 打包（pnpm tauri build --bundles nsis，约 10-30 分钟）...
call pnpm tauri build --bundles nsis
if errorlevel 1 goto fail

echo.
echo ============================================================
echo [+] 构建完成
echo     EXE    : %CD%\src-tauri\target\release\AudioScript.exe
echo     安装包 : %CD%\src-tauri\target\release\bundle\nsis\
dir /b "src-tauri\target\release\bundle\nsis\*.exe" 2>nul
echo.
echo 首次启动后日志应出现：OBS 浏览器源已就绪：http://127.0.0.1:17890/overlay
echo 验证：curl http://127.0.0.1:17890/health
echo ============================================================
pause
exit /b 0

:need_node
echo [x] 没有 Node.js。先执行：winget install OpenJS.NodeJS.LTS
goto fail
:need_pnpm
echo [x] 没有 pnpm。先执行：npm i -g pnpm@10
goto fail
:need_rust
echo [x] 没有 Rust。先执行：winget install Rustlang.Rustup
goto fail
:need_vs
echo [x] 切换 MSVC 失败，通常是没装 VS 2022 Build Tools 的 C++ 工作负载。
echo     见 BUILD-WINDOWS.md 第 1 节。
goto fail

:fail
echo.
echo [x] 构建失败。按顺序排查：
echo   0. 没装环境？先右键运行 setup-build-env.bat（管理员）。
echo   1. VS 2022 Build Tools 勾了「使用 C++ 的桌面开发」和 Windows SDK？
echo   2. rustup default stable-msvc 且 rustc -vV 的 host 是 x86_64-pc-windows-msvc？
echo   3. RUSTFLAGS 是否为 -C target-feature=+crt-static（ct2rs/sherpa-onnx 用 /MT）？
echo   4. 模型下载失败：重跑 pnpm fetch:mt；公司网络需配代理或镜像。
echo   5. sherpa-onnx 预编译库缺失：删除 src-tauri\target\sherpa-onnx-prebuilt 后重试。
echo   6. 仍失败就把最后 30 行日志发我。
pause
exit /b 1
