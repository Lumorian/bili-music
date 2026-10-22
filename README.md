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
cargo test -p bili-music-sync                        # 房间状态机（离线）
cargo test -p bili-music --test proxy                # 本地代理（离线）
cargo test -p bili-music --test live -- --nocapture  # 联网冒烟（B 站）
```

## 已知限制

- **B 站没有歌词接口**（多数视频也没有字幕），所以正在播放页展示的是
  视频简介和分 P 列表，而不是歌词。
- 一起听的房间状态保存在内存里，服务器重启后房间会消失（单机场景够用）。
- 官方接口随时可能调整风控（`-352`），代码里已做密钥失效自动重试，但仍需自行承担风险。

## 免责声明

本项目仅供学习与技术研究，所有内容版权归原平台与原作者所有。
请勿用于商业用途；因使用本项目产生的任何后果由使用者自行承担。
