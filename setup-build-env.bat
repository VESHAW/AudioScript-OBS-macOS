@echo off
chcp 65001 >nul
setlocal EnableExtensions EnableDelayedExpansion
cd /d "%~dp0"

echo ============================================================
echo  AudioScript Windows 编译环境 一键安装
echo  VS 2022 Build Tools（C++/SDK/CMake）+ Rust MSVC
echo  + Node.js 22 + pnpm 10 + Python 3.12（可选）
echo ============================================================
echo.

rem ---------- 必须是管理员 ----------
whoami /groups 2>nul | findstr /c:"S-1-16-12288" >nul
if errorlevel 1 (
  echo [x] 需要管理员权限。
  echo     右键 setup-build-env.bat → “以管理员身份运行”，再点一次。
  echo.
  pause
  exit /b 1
)

call :find_winget
set "WINGET="
for %%W in (winget.exe) do if not defined WINGET set "WINGET=%%~$PATH:W"
if not defined WINGET if exist "%LocalAppData%\Microsoft\WindowsApps\winget.exe" set "WINGET=%LocalAppData%\Microsoft\WindowsApps\winget.exe"
if not defined WINGET if exist "%SystemRoot%\System32\winget.exe" set "WINGET=%SystemRoot%\System32\winget.exe"
if not defined WINGET exit /b 0
"%WINGET%" --version >nul 2>&1
if errorlevel 1 (
  echo [warn] winget 存在但无法运行，改走官方安装包直装。
  set "WINGET="
)
exit /b 0


:download
rem %1 = URL, %2 = 目标文件。已存在则跳过。
if exist "%~2" exit /b 0
where curl.exe >nul 2>&1
if not errorlevel 1 (
  echo [*] 下载：%~nx2
  curl.exe -fL --retry 3 --connect-timeout 20 -o "%~2" "%~1"
  if not errorlevel 1 exit /b 0
  del "%~2" >nul 2>&1
)
echo [*] 改用 certutil 下载：%~nx2
certutil -urlcache -split -f "%~1" "%~2" >nul 2>&1
if exist "%~2" exit /b 0
echo [x] 下载失败：%~1
exit /b 1


:find_vs
set "VSPATH="
if exist "%VSWHERE%" (
  "%VSWHERE%" -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath > "%TEMP%\as_vs_path.txt" 2>nul
  set /p VSPATH=<"%TEMP%\as_vs_path.txt"
  del "%TEMP%\as_vs_path.txt" >nul 2>&1
)
exit /b 0


:direct_vs
echo [*] 下载 VS 2022 Build Tools 引导器（约 4 MB）...
set "VSB=%TEMP%\vs_BuildTools.exe"
call :download "https://aka.ms/vs/17/release/vs_BuildTools.exe" "%VSB%" || exit /b 1
echo [*] 启动 VS 安装器：VCTools + 推荐组件 + CMake...
echo     安装期间界面无进度，任务管理器里能看到 vs_installer。
"%VSB%" --wait --quiet --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --add Microsoft.VisualStudio.Component.VC.CMake.Project
echo [i] 安装器退出码：!errorlevel!（3010 表示需要重启）
exit /b 0


:ensure_vs
call :find_vs
if defined VSPATH (
  echo [+] 已有 VS C++ 工具链：!VSPATH!
  if exist "%SDKDIR%" (
    dir /b "%SDKDIR%" >nul 2>&1
    if not errorlevel 1 (
      echo [+] Windows SDK 已就位
      exit /b 0
    )
  )
  echo [warn] 有 C++ 工具链但缺 Windows SDK，追加安装...
)

echo [*] 安装/补装 VS 2022 Build Tools：VCTools + 推荐组件 + CMake...
if defined WINGET (
  "%WINGET%" install --id Microsoft.VisualStudio.2022.BuildTools -e ^
    --accept-package-agreements --accept-source-agreements ^
    --override "--wait --quiet --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --add Microsoft.VisualStudio.Component.VC.CMake.Project"
  echo [i] winget 退出码：!errorlevel!（提示“已安装”时非 0 属正常）
) else (
  call :direct_vs
)

call :find_vs
if not defined VSPATH (
  echo [x] VS C++ 工具链仍未检测到
  exit /b 1
)
echo [+] VS 安装完成：!VSPATH!
exit /b 0


:direct_rustup
echo [*] 下载 rustup-init.exe...
set "RUSTUP_INIT=%TEMP%\rustup-init.exe"
call :download "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe" "%RUSTUP_INIT%" || exit /b 1
echo [*] 安装 Rust stable-msvc（首次会下载标准库）...
"%RUSTUP_INIT%" -y --default-host x86_64-pc-windows-msvc --default-toolchain stable-msvc --profile default
if errorlevel 1 exit /b 1
exit /b 0


:ensure_rust
where rustc >nul 2>&1
if errorlevel 1 (
  echo [*] 安装 Rustup...
  if defined WINGET (
    "%WINGET%" install --id Rustlang.Rustup -e --accept-package-agreements --accept-source-agreements
  ) else (
    call :direct_rustup || exit /b 1
  )
)
set "RUSTUP=%USERPROFILE%\.cargo\bin\rustup.exe"
if exist "%RUSTUP%" (
  echo [*] 切换 MSVC 工具链（首次会下载标准库）...
  "%RUSTUP%" default stable-msvc >nul 2>&1
  "%RUSTUP%" component add rustfmt >nul 2>&1
)
call :refresh_path
where rustc >nul 2>&1
if errorlevel 1 (
  echo [x] rustc 不可用：重开一个 cmd 再跑本脚本
  exit /b 1
)
for /f "tokens=2" %%h in ('rustc -vV ^| findstr /b "host:"') do set "RHOST=%%h"
echo [i] rustc host: !RHOST!
if /i not "!RHOST!"=="x86_64-pc-windows-msvc" (
  echo [x] 不是 MSVC 工具链，请执行：rustup default stable-msvc
  exit /b 1
)
exit /b 0


:direct_node
echo [*] 查询 Node.js 22 最新安装包...
set "NODE_SUMS=%TEMP%\node_shasums.txt"
call :download "https://nodejs.org/dist/latest-v22.x/SHASUMS256.txt" "%NODE_SUMS%" || exit /b 1
set "NODE_MSI="
for /f "tokens=2" %%F in ('findstr /i "win-x64.msi" "%NODE_SUMS%"') do set "NODE_MSI=%%F"
if not defined NODE_MSI (
  echo [x] 没能解析出 Node.js 22 的 MSI 文件名
  exit /b 1
)
call :download "https://nodejs.org/dist/latest-v22.x/!NODE_MSI!" "%TEMP%\!NODE_MSI!" || exit /b 1
echo [*] 静默安装 !NODE_MSI!...
msiexec /i "%TEMP%\!NODE_MSI!" /qn /norestart
set "NODE_RC=!errorlevel!"
if !NODE_RC! NEQ 0 (
  if !NODE_RC! NEQ 3010 (
    echo [x] Node MSI 安装失败，退出码 !NODE_RC!
    echo     可手动下载安装：https://nodejs.org/en/download
    exit /b 1
  )
)
exit /b 0


:ensure_node
set "NODE_MAJOR=0"
where node >nul 2>&1
if not errorlevel 1 (
  for /f "tokens=1 delims=v." %%v in ('node -v') do set "NODE_MAJOR=%%v"
  echo [+] 已有 Node !NODE_MAJOR!
)
if !NODE_MAJOR! LSS 22 (
  echo [*] 安装 Node.js 22（项目 CI 固定 22.x）...
  if defined WINGET (
    "%WINGET%" install --id OpenJS.NodeJS.22 -e --accept-package-agreements --accept-source-agreements --force
  ) else (
    call :direct_node || exit /b 1
  )
)
call :refresh_path
where node >nul 2>&1
if errorlevel 1 (
  echo [x] node 不可用
  exit /b 1
)
for /f "tokens=1 delims=v." %%v in ('node -v') do set "NODE_MAJOR=%%v"
if !NODE_MAJOR! LSS 22 (
  echo [x] Node 版本仍低于 22，请手动安装 Node.js 22 后重跑
  exit /b 1
)
echo [+] Node:
node -v
exit /b 0


:ensure_pnpm
set "PNPM_MAJOR=0"
where pnpm >nul 2>&1
if not errorlevel 1 (
  for /f "tokens=1 delims=." %%v in ('pnpm -v') do set "PNPM_MAJOR=%%v"
)
if !PNPM_MAJOR! LSS 10 (
  echo [*] 安装 pnpm 10.29.2（与 packageManager 字段一致）...
  call npm i -g pnpm@10.29.2
)
call :refresh_path
where pnpm >nul 2>&1
if errorlevel 1 (
  echo [x] pnpm 不可用。重开 cmd 后重跑；或手动：npm i -g pnpm@10.29.2
  exit /b 1
)
echo [+] pnpm:
pnpm -v
exit /b 0


:ensure_python
where py >nul 2>&1
if not errorlevel 1 exit /b 0
where python >nul 2>&1
if not errorlevel 1 exit /b 0
if defined WINGET (
  echo [*] 安装 Python 3.12（官方 CI 同版本，构建脚本可能用到）...
  "%WINGET%" install --id Python.Python.3.12 -e --accept-package-agreements --accept-source-agreements
  call :refresh_path
) else (
  echo [i] 无 winget，跳过 Python（不影响编译；需要时手动装 python.org 3.12）
)
exit /b 0


:verify
echo.
echo ---------- 环境自检 ----------
where node >nul 2>&1
if errorlevel 1 (
  echo [x] node 缺失
  exit /b 1
)
node -v

where pnpm >nul 2>&1
if errorlevel 1 (
  echo [x] pnpm 缺失
  exit /b 1
)
pnpm -v

where cargo >nul 2>&1
if errorlevel 1 (
  echo [x] cargo 缺失
  exit /b 1
)
cargo -V

echo [i] rustc host:
rustc -vV | findstr /b "host:"

echo [i] MSVC:
if exist "%VSWHERE%" "%VSWHERE%" -latest -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property displayName

where py >nul 2>&1
if errorlevel 1 (
  echo [i] Python 未装，通常不影响编译
) else (
  py -3 -V
)
echo ------------------------------
exit /b 0


:fail
echo.
echo [x] 环境安装未完成。逐项排查：
echo   1. 是否以管理员身份运行
echo   2. 网络能否访问 nodejs.org / static.rust-lang.org / aka.ms
echo   3. VS 安装日志：%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\*.log
echo   4. 装完重开 cmd，再执行：rustup default stable-msvc
echo   5. 实在不行，手动装 Node 22 和 rustup 后重跑本脚本
echo.
pause
exit /b 1
