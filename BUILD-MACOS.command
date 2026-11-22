#!/bin/bash
# AudioScript OBS macOS 一键构建（Apple Silicon）
# 用法：在“终端”里执行  bash BUILD-MACOS.command
# 或者右键 → 打开（首次可能被 Gatekeeper 拦，右键打开即可）

set -euo pipefail
cd "$(dirname "$0")"

echo "============================================================"
echo " AudioScript OBS macOS 构建（Apple Silicon / arm64）"
echo "============================================================"
echo

fail() {
  echo
  echo "[x] $1"
  exit 1
}

# ---------- 1. 基本检查 ----------
if ! xcode-select -p >/dev/null 2>&1; then
  echo "[x] 没有 Xcode Command Line Tools。先执行："
  echo "    xcode-select --install"
  echo "    弹窗里点“安装”，装完重新运行本脚本。"
  exit 1
fi

ARCH="$(uname -m)"
if [ "$ARCH" != "arm64" ]; then
  fail "当前是 $ARCH，不是 Apple Silicon。本脚本只构建 arm64 版。"
fi

if ! command -v node >/dev/null 2>&1; then
  fail "没找到 node。先装 Node 22：
    brew install node@22
    export PATH=\"\$(brew --prefix node@22)/bin:\$PATH\""
fi

NODE_MAJOR="$(node -v | sed 's/^v//' | cut -d. -f1)"
if [ "$NODE_MAJOR" -lt 22 ]; then
  fail "Node 版本是 $(node -v)，需要 >= 22。执行：
    brew install node@22
    export PATH=\"\$(brew --prefix node@22)/bin:\$PATH\""
fi

if ! command -v pnpm >/dev/null 2>&1; then
  fail "没找到 pnpm。执行：npm install -g pnpm@10.29.2"
fi

if ! command -v cargo >/dev/null 2>&1; then
  fail "没找到 cargo。执行：
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    source \"\$HOME/.cargo/env\""
fi

echo "[+] Xcode CLT: $(xcode-select -p)"
echo "[+] 架构: $ARCH"
echo "[+] Node: $(node -v)"
echo "[+] pnpm: $(pnpm -v)"
echo "[+] Rust: $(rustc -V)"
echo

# ---------- 2. 依赖 ----------
echo "[*] 安装前端依赖（pnpm install --frozen-lockfile）..."
pnpm install --frozen-lockfile

if ! command -v typeshare >/dev/null 2>&1 || ! typeshare --version 2>/dev/null | grep -q "1.13.4"; then
  echo "[*] 安装 typeshare-cli 1.13.4（首次约 3-10 分钟）..."
  cargo install typeshare-cli --version 1.13.4 --locked
fi

# ---------- 3. 模型 ----------
echo "[*] 下载语音识别模型 + VAD（约 150-250 MB）..."
pnpm fetch:model

echo
read -r -p "是否同时下载翻译模型？体积较大、首次约 10-20 分钟 [y/N]: " WITH_MT
case "$WITH_MT" in
  y|Y)
    if ! python3 -c 'import sys; sys.exit(0 if sys.version_info >= (3, 10) else 1)' 2>/dev/null; then
      echo "[warn] python3 不是 3.10+，翻译模型会失败，先跳过。"
      echo "       要翻译就先装 Python 3.12 并把 python3 指向它："
      echo "       brew install python@3.12"
      echo "       export PATH=\"\$(brew --prefix python@3.12)/libexec/bin:\$PATH\""
    else
      echo "[*] python3: $(python3 -V)"
      pnpm fetch:mt
    fi
    ;;
esac

# ---------- 4. 编译 ----------
echo
echo "[*] 开始编译（release，首次约 20-40 分钟）..."
echo "    不要设置 RUSTFLAGS；静态 CRT 只用于 Windows。"
pnpm tauri build --bundles dmg

APP="src-tauri/target/release/bundle/macos/AudioScript.app"
DMG_DIR="src-tauri/target/release/bundle/dmg"

echo
echo "============================================================"
if [ -d "$APP" ]; then
  echo "[+] 完成：$APP"
  lipo -info "$APP/Contents/MacOS/AudioScript" || true
fi
if [ -d "$DMG_DIR" ]; then
  echo "[+] DMG：$DMG_DIR"
  ls -lh "$DMG_DIR"
fi
echo "============================================================"
echo
echo "打开 DMG 所在文件夹..."
open "$DMG_DIR" 2>/dev/null || true
