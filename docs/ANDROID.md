# Android 构建指南

给这台 Arch Linux 机器从零装好 Android SDK / NDK 并打出 APK 的完整记录。
Windows / macOS 的差异在最后一节。

## 需要装什么

| 组件 | 版本 | 用途 |
| --- | --- | --- |
| JDK | 21（17 也可以） | Gradle 跑构建 |
| Android SDK Platform | **android-37** | Tauri 模板的 `compileSdk = 37` |
| Android SDK Build-Tools | **37.0.0** | aapt2 / d8 / apksigner |
| Android SDK Platform-Tools | 37.x | adb（装机、看日志） |
| Android NDK | **27.3.13750724**（r27d，≥ 27.0） | Rust 交叉编译用的 clang 与 sysroot |
| Rust targets | `aarch64-linux-android` 等 | 交叉编译目标 |
| Gradle | 9.6.1 | 由 wrapper 自动下载 |

> Tauri CLI 会强制检查 NDK 版本：低于 **27.0.0** 直接报
> `At least NDK 27.0.0 is required`。

## 一、装 JDK

```bash
sudo pacman -S jdk-openjdk        # Arch
# 或 Debian/Ubuntu: sudo apt install openjdk-21-jdk
export JAVA_HOME=/usr/lib/jvm/java-21-openjdk
```

确认：`java -version` 能打印 21。

## 二、装 Android SDK

官方推荐 Android Studio，但纯命令行更省事，也更容易在服务器 / 容器里复现。

### 1. 下载 command line tools

```bash
export ANDROID_HOME="$HOME/Android/Sdk"
mkdir -p "$ANDROID_HOME/cmdline-tools"
cd "$ANDROID_HOME"
curl -sSLo cmdline.zip https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip
unzip -q cmdline.zip -d cmdline-tools
mv cmdline-tools/cmdline-tools cmdline-tools/latest
rm cmdline.zip
export PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"
```

目录结构必须是 `cmdline-tools/latest/bin/sdkmanager`，少一层 `latest` 会找不到。

### 2. 接受许可

```bash
yes | sdkmanager --licenses
```

### 3. 装平台、构建工具与 NDK

```bash
sdkmanager "platform-tools" "platforms;android-37.0" "build-tools;37.0.0" "ndk;27.3.13750724"
```

`sdkmanager --list` 确认已装：

```bash
sdkmanager --list_installed
```

### 4. 设置环境变量

写进 `~/.bashrc` / `~/.zshrc`：

```bash
export ANDROID_HOME="$HOME/Android/Sdk"
export NDK_HOME="$ANDROID_HOME/ndk/27.3.13750724"
export PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"
```

> **踩坑**：`sdkmanager` 会把下载缓存写到 `~/.android/cache`。
> 如果 `$HOME` 不可写（容器 / 沙箱 / 只读挂载），会刷一堆
> `java.nio.file.NoSuchFileException: ~/.android/cache/...`，
> 然后报 "Failed to download any source lists"。解决办法是给它一个可写的 home：
>
> ```bash
> export ANDROID_USER_HOME=/some/writable/dir/.android
> ```

## 三、装 Rust 的 Android 目标

如果你用的是发行版自带的 `rustc`（Arch 的 `rust` 包），它**没有** rustup，
也就没有 `rustup target add`。两种办法：

**A. 换成 rustup（推荐）**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add aarch64-linux-android armv7-linux-androideabi
```

**B. 用发行版包**：Arch 上装 `rustup` 包即可，
或者装 `rust-aarch64-linux-android` 这类交叉标准库包（版本要跟 rustc 对上）。

## 四、告诉 Cargo 用 NDK 的 clang

这一步最容易漏。`cc` crate 默认找不到 `aarch64-linux-android-clang`，
编译 `ring` / `libsqlite` 之类会直接炸：

```
error occurred in cc-rs: failed to find tool "aarch64-linux-android-clang"
```

在 `~/.cargo/config.toml`（或项目里的 `$CARGO_HOME/config.toml`）加：

```toml
[target.aarch64-linux-android]
linker = "/path/to/ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android24-clang"
ar     = "/path/to/ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar"

[target.armv7-linux-androideabi]
linker = "/path/to/ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/armv7a-linux-androideabi24-clang"
ar     = "/path/to/ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar"
```

路径里的 `24` 是 **minSdk**（Tauri 模板里 `minSdk = 24`），要和
`src-tauri/gen/android/app/build.gradle.kts` 保持一致。

也可以不改配置，改用环境变量：

```bash
export CC_aarch64_linux_android="$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android24-clang"
export AR_aarch64_linux_android="$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar"
```

## 五、生成 Android 工程并打包

```bash
npm install
npm run android:init      # 只跑一次，生成 src-tauri/gen/android
npm run android:build     # 产出 APK
```

只打 arm64（绝大多数手机）会快很多：

```bash
npx tauri android build --apk --target aarch64
```

产物在：

```
src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk
src-tauri/gen/android/app/build/outputs/apk/arm64/debug/app-arm64-debug.apk
```

装到手机：

```bash
adb install -r src-tauri/gen/android/app/build/outputs/apk/arm64/debug/app-arm64-debug.apk
```

### Gradle 下载太慢怎么办

`services.gradle.org` 在某些网络下只有几十 KB/s，而 Gradle 发行包有 140 MB。
换成镜像：编辑 `src-tauri/gen/android/gradle/wrapper/gradle-wrapper.properties`

```properties
distributionUrl=https\://mirrors.cloud.tencent.com/gradle/gradle-9.6.1-bin.zip
```

Maven 依赖走 `repo1.maven.org`，一般够快；如果也慢，在
`src-tauri/gen/android/build.gradle.kts` 的 `repositories` 里把
`maven { url = uri("https://maven.aliyun.com/repository/public") }` 加到最前面。

## 六、调试

```bash
npm run android:dev        # 编译 + 装到设备 + 热重载
adb logcat | grep -i bili  # 看 App 日志（tauri-plugin-log 会打到 stderr）
```

- 界面在窄屏会自动切到底部标签栏布局（断点 900px）。
- 本地音频代理同样跑在 `127.0.0.1`，不需要额外配置。
- 调试构建允许明文 HTTP（`usesCleartextTraffic=true`），
  release 构建会关掉；如果你的一起听服务器是 http 的，release 包需要自己改
  这个 manifest 占位符或给它套 HTTPS。

## 七、常见错误

| 报错 | 原因 | 解决 |
| --- | --- | --- |
| `Android SDK not found` | `ANDROID_HOME` 没设 / 路径错 | 设成 SDK 根目录（含 `platforms/` 那一层） |
| `Failed to lookup version of installed NDK` | NDK 目录缺 `source.properties` | 用 sdkmanager 装，别手动解压 |
| `At least NDK 27.0.0 is required` | NDK 太旧 | 装 27.3.13750724 |
| `failed to find tool "aarch64-linux-android-clang"` | 没配 linker | 见第四节 |
| `Unsupported class file major version` | JDK 太老 | 用 JDK 17+（推荐 21） |
| Gradle 卡在 `Downloading gradle-9.6.1-bin.zip` | 官方源慢 | 换镜像（见上） |
| `SDK location not found` | 缺 `local.properties` | 设 `ANDROID_HOME`，或写 `sdk.dir=` 到 `local.properties` |

## 八、其他平台

- **Windows**：装 Android Studio（自带 SDK/NDK），
  `rustup target add aarch64-linux-android`，linker 配成
  `.../ndk/27.3.13750724/toolchains/llvm/prebuilt/windows-x86_64/bin/aarch64-linux-android24-clang.cmd`。
- **macOS**：`brew install --cask android-commandlinetools`，
  NDK 路径在 `~/Library/Android/sdk/ndk/<ver>/toolchains/llvm/prebuilt/darwin-x86_64/bin`。
