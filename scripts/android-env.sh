#!/usr/bin/env bash
# 把本仓库自带的一套 Android 工具链暴露到当前 shell。
#   source scripts/android-env.sh
#
# 为什么要有这个文件：这台机器上 $HOME 是只读的，
# 所以 SDK / NDK / rustup / gradle 的缓存全部落在仓库内部（已在 .gitignore 里）。

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

export ANDROID_HOME="$REPO_ROOT/.android-sdk"
export ANDROID_SDK_ROOT="$ANDROID_HOME"
export NDK_HOME="$ANDROID_HOME/ndk/27.3.13750724"

# sdkmanager / AGP 会往 $HOME/.android 写缓存，指向可写位置
export ANDROID_USER_HOME="$REPO_ROOT/.android-user-home"

export JAVA_HOME="${JAVA_HOME:-/usr/lib/jvm/java-21-openjdk}"

# rustup 的 toolchain 与 cargo 的缓存都留在仓库里
export RUSTUP_HOME="$REPO_ROOT/.rustup"
export CARGO_HOME="$REPO_ROOT/.cargo-home"
export RUSTUP_DIST_SERVER="https://mirrors.tuna.tsinghua.edu.cn/rustup"

export GRADLE_USER_HOME="$REPO_ROOT/.gradle-home"

export PATH="$CARGO_HOME/bin:$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"

echo "ANDROID_HOME   = $ANDROID_HOME"
echo "NDK_HOME       = $NDK_HOME"
echo "JAVA_HOME      = $JAVA_HOME"
echo "rustc          = $(command -v rustc)"
echo "sdkmanager     = $(command -v sdkmanager)"
