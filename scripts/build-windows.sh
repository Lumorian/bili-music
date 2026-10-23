#!/usr/bin/env bash
# 在 Linux 上交叉编译出 Windows 安装包（NSIS .exe）。
#   ./scripts/build-windows.sh
#
# 为什么能这么干：Tauri 2 支持 --runner cargo-xwin，
# 用 xwin 下载微软官方的 MSVC CRT/SDK 头文件和库，用 lld-link 链接，
# 全程不需要装 Visual Studio，也不需要 wine。
#
# 三个坑：
#   1. $HOME 只读 → cargo-xwin 和 tauri 的缓存目录必须搬到仓库里；
#   2. Tauri 在非 Windows 主机上直接调 PATH 里的 `makensis`（还会 env_remove NSISDIR），
#      所以要么装了系统 NSIS，要么这里自备一份并包一层 wrapper 设置 NSISDIR；
#   3. 打 NSIS 包前 Tauri 要往自己的 NSIS 目录塞一个插件 nsis_tauri_utils.dll，
#      而它**只会从 GitHub Releases 拉**。直连不通时报
#        failed to bundle project: `io: peer closed connection without sending TLS close_notify`
#      编译了半天最后一步才失败，很浪费。所以下面先自己取一份、校验 SHA1 再喂给它。
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"
# shellcheck disable=SC1091
source ./scripts/android-env.sh > /dev/null

TARGET="x86_64-pc-windows-msvc"
export XWIN_CACHE_DIR="$REPO_ROOT/.xwin-cache"
export XDG_CACHE_HOME="$REPO_ROOT/.tmpcache"
export PATH="$REPO_ROOT/.nsis/sys:$CARGO_HOME/bin:$PATH"
export NSIS_PATH="$REPO_ROOT/.nsis/sys"

# Tauri 找这个插件的目录（XDG_CACHE_HOME 上面已经指到仓库里了）
TAURI_NSIS_DIR="$XDG_CACHE_HOME/tauri/nsis"
# 与 tauri-cli 内置的期望值一致（从 cli.linux-x64-gnu.node 里读出来的常量）
NSIS_UTILS_SHA1="75197fee3c6a814fe035788d1c34ead39349b860"
NSIS_UTILS_NAME="nsis_tauri_utils.dll"
# 官方地址优先，其次几个 GitHub 反代；反代挂了就换下一个
NSIS_UTILS_URLS=(
  "https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/$NSIS_UTILS_NAME"
  "https://ghproxy.net/https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/$NSIS_UTILS_NAME"
  "https://gh-proxy.com/https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/$NSIS_UTILS_NAME"
  "https://ghfast.top/https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/$NSIS_UTILS_NAME"
)

# 已经把插件放到指定目录了吗？（同时看两个候选路径，见函数末尾的说明）
nsis_utils_present() {
  local d
  for d in "$TAURI_NSIS_DIR/Plugins/x86-unicode/additional" "$TAURI_NSIS_DIR/x86-unicode/additional"; do
    if [ -f "$d/$NSIS_UTILS_NAME" ] && \
       [ "$(sha1sum "$d/$NSIS_UTILS_NAME" | cut -d' ' -f1)" = "$NSIS_UTILS_SHA1" ]; then
      return 0
    fi
  done
  return 1
}

# 取一份带正确 SHA1 的插件，成功时打印它的路径
fetch_nsis_utils() {
  local tmp="$REPO_ROOT/.tmpcache/$NSIS_UTILS_NAME.download"
  local url sum
  mkdir -p "$(dirname "$tmp")"
  for url in "${NSIS_UTILS_URLS[@]}"; do
    # -f：HTTP 错误码当失败（否则反代会把错误页当 dll 存下来）
    if ! curl -fsSL --retry 2 --connect-timeout 15 -m 120 -o "$tmp" "$url"; then
      echo "    ✗ $url" >&2
      continue
    fi
    sum="$(sha1sum "$tmp" | cut -d' ' -f1)"
    if [ "$sum" != "$NSIS_UTILS_SHA1" ]; then
      echo "    ✗ $url（SHA1 不匹配：$sum）" >&2
      continue
    fi
    echo "    ✓ $url" >&2
    echo "$tmp"
    return 0
  done
  rm -f "$tmp"
  return 1
}

echo "==> 1/5 准备 Rust 目标"
rustup target add "$TARGET"
command -v cargo-xwin >/dev/null || cargo install --locked cargo-xwin

echo "==> 2/5 准备 NSIS（Tauri 打 installer 要用）"
if [ ! -x "$REPO_ROOT/.nsis/sys/makensis" ]; then
  echo "缺少 $REPO_ROOT/.nsis/sys/makensis"
  echo "做法：从 Debian 仓库下 nsis + nsis-common 两个 deb，解包到 .nsis/usr，"
  echo "      再把 usr/share/nsis 的内容拷进 .nsis/sys，并放一个设置 NSISDIR 的 makensis wrapper。"
  exit 1
fi

echo "==> 3/5 预置 NSIS 插件 $NSIS_UTILS_NAME"
if nsis_utils_present; then
  echo "    已就位，跳过下载"
elif [ -n "${BILIMUSIC_OFFLINE:-}" ]; then
  echo "    设置了 BILIMUSIC_OFFLINE，但本地没有这个插件。" >&2
  echo "    把它放到下列任一目录后重试：" >&2
  echo "      $TAURI_NSIS_DIR/Plugins/x86-unicode/additional/$NSIS_UTILS_NAME" >&2
  echo "      $TAURI_NSIS_DIR/x86-unicode/additional/$NSIS_UTILS_NAME" >&2
  exit 1
else
  echo "    本地缓存里没有，开始下载（直连 github 常常不通，会依次试镜像）"
  if ! dl="$(fetch_nsis_utils)"; then
    echo "    所有镜像都失败了。" >&2
    echo "    手动下载这个文件，校验 SHA1 = $NSIS_UTILS_SHA1，然后放进：" >&2
    echo "      $TAURI_NSIS_DIR/Plugins/x86-unicode/additional/$NSIS_UTILS_NAME" >&2
    exit 1
  fi
  # 两个位置都放：不同 tauri-cli 版本认的路径不一样
  # （插件相对 NSIS 根目录，有的版本带 Plugins/ 前缀，有的不带）
  mkdir -p "$TAURI_NSIS_DIR/Plugins/x86-unicode/additional" "$TAURI_NSIS_DIR/x86-unicode/additional"
  cp -f "$dl" "$TAURI_NSIS_DIR/Plugins/x86-unicode/additional/$NSIS_UTILS_NAME"
  cp -f "$dl" "$TAURI_NSIS_DIR/x86-unicode/additional/$NSIS_UTILS_NAME"
  rm -f "$dl"
  echo "    已就位"
fi

echo "==> 4/5 交叉编译 + 打 NSIS 安装包"
npx tauri build --runner cargo-xwin --target "$TARGET" --bundles nsis

echo "==> 5/5 收集产物"
OUT="$REPO_ROOT/dist-packages"
mkdir -p "$OUT"
cp -f "target/$TARGET/release/bundle/nsis/"*setup.exe "$OUT/"
cp -f "target/$TARGET/release/bili-music.exe" "$OUT/bili-music-0.1.0-windows-x64.exe"

cd "$OUT"
sha256sum BiliMusic* bili-music* > SHA256SUMS.txt
sha256sum -c SHA256SUMS.txt
ls -lh
