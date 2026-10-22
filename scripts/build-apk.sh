#!/usr/bin/env bash
# 一条命令打出 Android APK。
#   ./scripts/build-apk.sh              # debug，arm64
#   ./scripts/build-apk.sh release      # release，arm64
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"
# shellcheck disable=SC1091
source ./scripts/android-env.sh > /dev/null

MODE="${1:-debug}"
case "$MODE" in
  debug)   FLAGS="--debug" ;;
  release) FLAGS="" ;;
  *) echo "用法: $0 [debug|release]"; exit 1 ;;
esac

if [ ! -d src-tauri/gen/android ]; then
  echo "==> 首次构建，生成 Android 工程"
  npx tauri android init --ci
fi

echo "==> 构建 APK ($MODE, arm64)"
npx tauri android build --apk --target aarch64 $FLAGS

find src-tauri/gen/android/app/build/outputs/apk -name '*.apk' -exec ls -lh {} \;
