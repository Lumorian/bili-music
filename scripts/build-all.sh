#!/usr/bin/env bash
# 一条命令打出全部发布产物（Linux deb/rpm/AppImage + Android APK）。
#   ./scripts/build-all.sh
#
# 产物统一放到 dist-packages/，并生成 SHA256SUMS.txt。
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"
# shellcheck disable=SC1091
source ./scripts/android-env.sh > /dev/null

OUT="$REPO_ROOT/dist-packages"
mkdir -p "$OUT"

echo "==> 1/4 前端类型检查 + 构建"
npx tsc --noEmit
npx vite build

echo "==> 2/4 Linux 打包（deb / rpm / AppImage）"
APPIMAGE_EXTRACT_AND_RUN=1 npx tauri build --bundles deb,rpm,appimage

echo "==> 3/4 一起听房间服务器"
cargo build --release -p bili-music-sync-server

echo "==> 4/4 Android APK（arm64，release 签名）"
if [ -f "$REPO_ROOT/.android-keys/bilimusic.jks" ]; then
  export BILIMUSIC_SIGNING_STORE_FILE="$REPO_ROOT/.android-keys/bilimusic.jks"
  export BILIMUSIC_SIGNING_STORE_PASSWORD="bilimusic"
  export BILIMUSIC_SIGNING_KEY_ALIAS="bilimusic"
  export BILIMUSIC_SIGNING_KEY_PASSWORD="bilimusic"
fi
# Gradle 在 PATH 里找不到 NDK 的 clang，必须显式指过去
NDK_BIN="$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin"
export PATH="$NDK_BIN:$PATH"
export CC_aarch64_linux_android="$NDK_BIN/aarch64-linux-android24-clang"
export AR_aarch64_linux_android="$NDK_BIN/llvm-ar"
npx tauri android build --apk --target aarch64

echo "==> 收集产物"
cp -f target/release/bundle/deb/*.deb "$OUT/"
cp -f target/release/bundle/rpm/*.rpm "$OUT/"
cp -f target/release/bundle/appimage/*.AppImage "$OUT/"
cp -f target/release/bili-music "$OUT/bili-music-0.1.0-linux-x86_64"
cp -f target/release/bili-music-sync-server "$OUT/"
cp -f src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk \
      "$OUT/BiliMusic_0.1.0_arm64.apk"

cd "$OUT"
rm -f SHA256SUMS.txt
sha256sum BiliMusic* bili-music* > SHA256SUMS.txt
sha256sum -c SHA256SUMS.txt

echo
echo "完成，产物在 $OUT"
ls -lh
