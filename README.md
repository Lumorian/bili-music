# BiliMusic

> 只用来**听**的哔哩哔哩第三方客户端 —— 把视频画面整个剥掉，只留音轨，
> 再带一个「和好友一起听」的房间功能。

支持 **GNU/Linux · Windows · Android**，桌面端与手机端共用一套 Rust 后端 + React 前端。

![BiliMusic 主界面](docs/screenshot-home.png)

---

## 它是什么

B 站官方的"听视频"很好用，但你必须开着视频页面、让播放器一直解码画面。
BiliMusic 做的事情很简单也很彻底：

- **只请求 dash 里的纯音频轨**（`fnval=16` 的 `audio` 流），不下载、不解码任何视频帧。
  一首 4 分钟的歌通常只有 4~8 MB，内存占用几十 MB 级别。
- 界面是**音乐播放器**的形态：榜单、搜索、收藏、最近播放、队列、正在播放页。
  没有弹幕、没有画面、没有推荐信息流。
- **一起听**：**不用自建服务器** —— 点「创建房间」直接出一个 6 位房间号，
  把号发给朋友就能进房；切歌 / 暂停 / 拖动进度 / 加歌 / 聊天全部实时同步。
  想脱离公共通道可以切到自建服务器模式，两种模式行为完全一致。

## 功能一览

| 模块 | 功能 |
| --- | --- |
| B 站 | 音乐区/演奏/翻唱/原创/VOCALOID 等分区排行、关键词搜索、相关推荐、收藏夹、稍后再看、扫码登录 |
| 播放 | 纯音频流、本地代理（补 Referer / 透传 Range 支持秒拖进度）、顺序/循环/单曲/随机、音量、全局快捷键（空格、Shift+←/→） |
| 一起听 | 云房间（无需服务器，走公共 MQTT 通道）与自建服务器两种模式；队列同步、播放状态同步（带服务器时间补偿）、成员进度小条、房主移交、踢人、房间聊天 |
| 账号 | 侧边栏常驻账号入口（头像/昵称/未登录提示），扫码登录、退出登录；登录态以 `SESSDATA` 为准，风控导致的 `nav` 失败不会把用户误判为未登录 |
| 其它 | 本地收藏与最近播放、深色 UI、窄屏/手机自适应布局 |

## 架构

```
┌──────────────────────────────────────────────────────────────┐
│  React + TypeScript 前端 (src/)                               │
│  ├── store/player.ts    播放状态机                            │
│  ├── store/together.ts  一起听状态 + 服务器时钟偏移             │
│  └── lib/audio.ts       唯一的 <audio> 元素，负责出声与对齐      │
└───────────────┬──────────────────────────────┬───────────────┘
                │ invoke / event               │
┌───────────────▼──────────────────────────────▼───────────────┐
│  Tauri 2 后端 (src-tauri/)                                    │
│  ├── bilibili.rs  wbi 签名、搜索、排行、playurl(dash audio)     │
│  ├── proxy.rs     本地音频代理 127.0.0.1:<随机端口>/stream/<tok> │
│  ├── session.rs   登录态 / 设置持久化                           │
│  └── together.rs  一起听客户端（把 sync-core 接进 Tauri）        │
└───────────────┬──────────────────────────────────────────────┘
                │
┌───────────────▼──────────────────────────────────────────────┐
│  sync-core/  一起听协议与房间状态机（桌面端 / Android 共用）      │
│  server/     可独立部署的房间服务器（axum + WebSocket）          │
└──────────────────────────────────────────────────────────────┘
```

**为什么要本地代理？**
B 站音频 CDN 会校验 Referer，且返回的是 `http` 地址；直接塞进 WebView 的
`<audio>` 会 403 或被混合内容策略拦掉。代理顺手还解决了 **Range 透传**，
所以拖动进度条是秒响应的，而不是等到整首歌缓冲完。

## 快速开始（桌面端）

前置依赖见 [Tauri 官方文档](https://tauri.app/start/prerequisites/)：
Linux 需要 `webkit2gtk-4.1`、`libsoup3`、`gtk3`；Windows 需要 WebView2（Win10/11 自带）；
另外需要 Node.js 20+ 与 Rust 1.85+。

```bash
npm install
npm run tauri:dev        # 开发模式
npm run tauri:build      # 打生产包（deb/rpm/AppImage / msi / exe）
npm run sync:server      # 顺便起一个一起听房间服务器
```

> 只想调 UI 的话，直接 `npm run dev` 在浏览器里打开即可 —— 没有 Tauri 运行时
> 会自动走内置的假数据后端，不用联网。

## 打包发布

一条命令出全部产物（Linux deb/rpm/AppImage + Android APK + 房间服务器）：

```bash
./scripts/build-all.sh
```

产物统一落在 `dist-packages/`，并生成 `SHA256SUMS.txt`：

| 文件 | 用途 |
| --- | --- |
| `BiliMusic_0.1.0_amd64.deb` | Debian / Ubuntu（3.4 MB，已声明 GStreamer 依赖） |
| `BiliMusic-0.1.0-1.x86_64.rpm` | Fedora / openSUSE（3.4 MB，已声明 GStreamer 依赖） |
| `BiliMusic_0.1.0_amd64.AppImage` | 免安装，自带依赖（含 GStreamer 音频插件，约 157 MB） |
| `bili-music-0.1.0-linux-x86_64` | 裸二进制（7.4 MB） |
| `BiliMusic_0.1.0_arm64.apk` | Android arm64（11 MB，release 已签名） |
| `BiliMusic_0.1.0_x64-setup.exe` | Windows 安装包（NSIS，3 MB） |
| `bili-music-0.1.0-windows-x64.exe` | Windows 免安装单文件（9.7 MB） |
| `bili-music-sync-server` | 自建一起听服务器（2 MB） |

单独打包：

```bash
npx tauri build --bundles deb              # 只出 deb
npx tauri build --bundles appimage         # 只出 AppImage
npx tauri build --bundles nsis             # 只出 Windows 安装包（需在 Windows 上跑）
npx tauri android build --apk --target aarch64   # 只出 APK
```

### 在 Linux 上交叉编译 Windows 安装包（`./scripts/build-windows.sh`）

```bash
./scripts/build-windows.sh
```

Tauri 2 支持 `--runner cargo-xwin`：xwin 会拉微软官方的 MSVC CRT/SDK，用 `lld-link` 链接，
**不需要装 Visual Studio，也不需要 wine**，最后用 NSIS 打出 `.exe` 安装包。

脚本里已经处理了这台机器的三个坑：

1. `$HOME` 只读 → `XWIN_CACHE_DIR` / `XDG_CACHE_HOME` 都改到仓库内部；
2. Tauri 在非 Windows 主机上直接调 PATH 里的 `makensis`（并且会 `env_remove("NSISDIR")`），
   所以自备了一份 NSIS 3.12 并包了一层 wrapper 自己设 `NSISDIR`（放在 `.nsis/sys/`，已 gitignore）。
3. 打 NSIS 包前，Tauri 要往它的 NSIS 目录里塞一个插件 `nsis_tauri_utils.dll`，
   而这个文件**只会从 GitHub Releases 下载**。直连不通时，编译到最后一步才报
   `failed to bundle project: io: peer closed connection without sending TLS close_notify`。
   脚本现在会先自己取一份、**校验 SHA1**（`75197fee…`，与 tauri-cli 内置期望值一致）再交给它，
   官方地址失败就依次换镜像（ghproxy / gh-proxy / ghfast）。
   - 想完全离线：把这个 dll 放进 `.tmpcache/tauri/nsis/Plugins/x86-unicode/additional/`，
     然后 `BILIMUSIC_OFFLINE=1 ./scripts/build-windows.sh`。
   - 所有镜像都失败时，脚本会打印手动放置路径并以非 0 退出，不会闷头失败。

> 注意：产物没有代码签名（`sign_command` 只在 Windows 主机上生效），
> 首次运行会被 SmartScreen 拦一下，需要点「更多信息 → 仍要运行」。
> 也由于是交叉编译，**未能在真实 Windows 上跑过**，仅验证了 PE 结构、DLL 导入和嵌入的前端资源。

> release APK 的签名密钥放在 `.android-keys/`（已 gitignore）。
> 首次生成：`keytool -genkeypair -keystore .android-keys/bilimusic.jks -alias bilimusic \
>   -keyalg RSA -keysize 2048 -validity 10000 -storepass bilimusic -keypass bilimusic \
>   -dname "CN=BiliMusic"`，然后靠环境变量 `BILIMUSIC_SIGNING_*` 传给 Gradle。

## 一起听

### 默认：云房间（不用自建任何东西）

1. 打开「一起听」，填个昵称；
2. 点「创建房间」，立刻得到 6 位房间号；
3. 把号发给朋友，他们在同一页输入房间号 + 自己的昵称即可进房。

原理：所有人连同一个公共 MQTT 通道（默认 `broker.emqx.io` 的免费节点），
按房间号划分话题；**房主的客户端代行服务器职责**，收到别人的操作后跑一遍和自建服务器
完全同一份状态机，再把结果广播出去。所以两种模式的同步语义一模一样。

代价写在明面上：公共通道是明文的（别聊隐私）、房主退出房间就散、依赖第三方可用性。

细节见 **[docs/TOGETHER.md](docs/TOGETHER.md)**。

### 可选：自建服务器

想长期固定小圈子用（房主退出房间不会散、通道只有你自己）：

```bash
npm run sync:server:release
# 或指定地址
BIND=0.0.0.0:9000 cargo run --release --manifest-path server/Cargo.toml
```

在「一起听 → 高级」里填服务器地址（本机 `http://127.0.0.1:8787`，
局域网填内网 IP，公网记得反代同时转发 WebSocket）。

服务器接口：

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| GET | `/health` | 健康检查 |
| POST | `/api/rooms` | 建房，body `{"name":"房间名","code":"可选"}` |
| GET | `/api/rooms/{code}` | 房间快照 |
| WS | `/ws/{code}?name=&avatar=` | 实时同步通道 |

协议见 `sync-core/src/protocol.rs` 与 [docs/PROTOCOL.md](docs/PROTOCOL.md)。

## Android

完整步骤（SDK / NDK / rustup 怎么装、踩过的坑、常见报错）见 **[docs/ANDROID.md](docs/ANDROID.md)**。

**本机已经装好了一套**，都在仓库目录里（已 gitignore），两条命令就能出包：

```bash
source scripts/android-env.sh   # 把 SDK / NDK / rustup / gradle 塞进当前 shell
./scripts/build-apk.sh          # 默认 debug + arm64；传 release 打正式包
```

项目里装的版本：

| 组件 | 版本 |
| --- | --- |
| Android SDK Platform | android-37.0（Tauri 模板 `compileSdk = 37`） |
| Build-Tools | 37.0.0 |
| NDK | 27.3.13750724（r27d） |
| Rust | 1.99.1 + `aarch64/armv7/i686/x86_64-linux-android` 标准库 |
| Gradle | 9.6.1（wrapper 走腾讯云镜像） |
| JDK | 21 |

产物在 `src-tauri/gen/android/app/build/outputs/apk/<abi>/<mode>/app-<abi>-<mode>.apk`，
其实测已打出 184 MB 的 debug 包（arm64）。

手机上本地音频代理同样跑在 `127.0.0.1`，不需要额外配置；界面在窄屏下自动切到底部标签栏。

## 目录结构

```
├── src/                 前端（React + TS）
│   ├── components/      Player / NowPlaying / QueuePanel / QrLogin / Toast / Icons
│   ├── pages/           Home / Search / Library / Together / Settings
│   ├── store/           player / together / library（zustand）
│   └── lib/             api（invoke 封装）/ audio（播放引擎）/ format / mock
├── src-tauri/           桌面端 + 安卓端 Rust 后端
│   ├── src/bilibili.rs  B 站接口（wbi 签名）
│   ├── src/proxy.rs     本地音频代理
│   ├── src/together*.rs 一起听接入
│   └── tests/           联网冒烟测试 + 代理测试
├── sync-core/           一起听协议、房间状态机、客户端连接器
└── server/              一起听房间服务器（独立部署）
```

## 测试

```bash
npx tsc --noEmit                                     # 前端类型检查
cargo test -p bili-music-sync                        # 房间状态机 + MQTT + ws 端到端
cargo test -p bili-music --test proxy                # 本地代理（离线）
cargo test -p bili-music --test live -- --nocapture  # 联网冒烟（B 站 + 代理取真实音频字节）
cargo test --workspace                               # 全部；联网用例内部自带串行锁，不会被风控
```

## 已知限制

- **B 站没有歌词接口**（多数视频也没有字幕），所以正在播放页展示的是
  视频简介和分 P 列表，而不是歌词。
- 一起听的房间状态保存在内存里，服务器重启后房间会消失（单机场景够用）。
- 官方接口随时可能调整风控（`-352`）。程序里的应对是分层的：自动申请并持久化
  `buvid3/buvid4` 设备指纹（缺失时 `ranking` 必被风控）、签名失效自动换密钥、
  风控时退避重试，最后还有 `ranking/region` 老接口兜底。即便如此仍可能偶发失败。
- **Linux 播放依赖系统的 GStreamer 音频插件**。WebKitGTK 的 `<audio>` 走 GStreamer，
  插件缺失时不只是没声音 —— 给 `audio.src` 赋值会直接让 WebProcess abort，
  表现是「一点播放整个窗口就消失」。
  - 用 deb/rpm 安装时依赖已声明，正常装包就会带上；
  - 用 AppImage 时已开启 `bundleMediaFramework`（体积会大 30MB 左右）；
  - 裸二进制运行需要自备：Debian/Ubuntu `gstreamer1.0-plugins-{base,good,bad,ugly} gstreamer1.0-libav`，
    Arch `gst-plugins-{base,good,bad,ugly} gst-libav`。
  - 前端启动时会用 `canPlayType` 探测一次解码能力，缺失时只弹提示、绝不碰 audio 元素，
    不会再崩。（设置页「关于」里能看到「音频解码后端：可用/缺失」。）

- **Windows（WebView2）与 Linux（WebKitGTK）的差异**：页面源一个是 `http://tauri.localhost`、
  一个是 `tauri://localhost`。前者会往图床请求里带 `Referer`（被 B 站 403，封面全灰），
  后者不带。所以 `index.html` 全局 `no-referrer`、所有 `<img>` 也各自声明了一遍。
  同样地，Chromium 的 Private Network Access 规则会让"页面 → 127.0.0.1 代理"多一次预检，
  本地代理已经显式应答，详情见 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#windows-上的两个坑)。

## 三个平台的差异（踩过的坑都在这）

| | Windows | Linux | Android |
| --- | --- | --- | --- |
| WebView | WebView2（Chromium） | WebKitGTK | 系统 WebView（Chromium） |
| 页面源 | `http://tauri.localhost` | `tauri://localhost` | `http://tauri.localhost` |
| 图片 Referer | 会带 → B 站图床 403 | 不带 → 正常 | 会带 → 403 |
| 明文 HTTP | 允许 | 允许 | release 默认**禁止**（含 loopback） |
| 音频解码 | 系统自带 | 依赖 GStreamer 插件 | 系统 WebView |

对应的处理：

- **图片**：`index.html` 里有 `<meta name="referrer" content="no-referrer">`，每个
  `<img>` 也各自带 `referrerPolicy="no-referrer"`（不依赖全局设置）。
- **明文 HTTP**：`res/xml/network_security_config.xml` 只对 `127.0.0.1 / localhost / ::1 /
  10.0.2.2` 放开，公网仍然强制 HTTPS。否则 release 包上"本地音频代理"会被系统拦掉，
  表现是**开发包能放、正式包一按播放就报错**。
- **音频解码**：探测时同时问 AAC(mp4a) 和 Opus(webm)，避免个别 Android WebView
  只报 webm 就被误判成"没有解码后端"。
- **本地代理**：补了 CORS 预检和 `Access-Control-Allow-Private-Network`
  （Chromium 的 Private Network Access 规则），`<audio>` 也不再设 `crossOrigin`。
- **安全区**：Android 用了 `enableEdgeToEdge`，`.app` 上用 `env(safe-area-inset-*)`
  把状态栏/导航栏的位置让出来，否则播放条会被压住。
- **手势**：进度条/音量条在触摸设备上要 `touch-action: none` + 28px 热区，
  滑块常显（`@media (pointer: coarse)`），不然手机上根本拖不动。

## 免责声明

本项目仅供学习与技术研究，所有内容版权归原平台与原作者所有。
请勿用于商业用途；因使用本项目产生的任何后果由使用者自行承担。
