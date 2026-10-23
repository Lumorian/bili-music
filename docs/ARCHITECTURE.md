# 架构说明

## 分层

```
前端 (src/)                      Rust 后端 (src-tauri/)
─────────────────                ──────────────────────
pages/    页面                   commands.rs   命令注册
components/ 播放器/队列/扫码      bilibili.rs   B 站接口（wbi 签名）
store/    zustand 状态            (仅 B 站接口)
lib/api   invoke 封装             proxy.rs      本地音频代理
lib/audio 单例 <audio>            session.rs    登录态持久化
                                  together.rs   一起听客户端
                                         │
                                   sync-core/  协议 + 房间状态机
                                   server/     独立房间服务器
```

## 为什么"只播音频"是可行的

B 站 `playurl` 接口（`fnval=16`）返回的 `dash` 里有独立的 `audio` 数组，
和视频流完全分开。取其中码率最高的一条就是音频轨（m4a / AAC），
整个过程不需要任何视频分片，也不需要解码器参与。

代价是：没有字幕、没有弹幕、没有画面。这正是这个项目想要的。

## 为什么需要本地代理

1. **Referer 校验**：B 站 CDN 对缺少 `Referer: https://www.bilibili.com` 的请求返回 403。
2. **混合内容**：上游有 http 地址，WebView 页面是 `tauri://` 或 `https://`，直接加载会被拦。
3. **Range 透传**：代理原样转发 `Range` 与 `Content-Range`，拖动进度条不用等整首歌下载完。

代理用 axum 起在 `127.0.0.1:<随机端口>`，每个流发一个随机 token，
`GET /stream/<token>` 时才去拉上游。token 只存在内存里，进程退出即失效。

## 一起听的时间对齐

客户端不直接"同步播放按钮"，而是同步**状态 + 时间戳**：

- 服务器记录 `(position_ms, updated_at, playing)`
- 客户端算出 `clockOffset = server_time - localNow`
- 期望位置 = `position_ms + (localNow + clockOffset - updated_at)`（播放中才累加）
- 与本地 `audio.currentTime` 偏差超过 1.2 秒才 seek，避免抖动；
  本地手动拖动后 2.5 秒内不纠偏（等服务器回声）。

这样即使网络延迟 300ms、甚至短暂掉线重连，体验上也几乎听不出不同步。

一个新歌的元数据（`loadedmetadata`）到手时会立刻按房间时间对齐一次，
不用等 2.5 秒的心跳，所以切歌瞬间也不会和队友错开。

## Windows 上的两个坑

### 封面全是灰块

B 站图床（`i0/i1/i2.hdslb.com`）校验 `Referer`：带了**非 bilibili 的 Referer 一律 403**，
不带 Referer 才放行。Linux 的页面源是 `tauri://localhost`（自定义协议，不发送 Referer），
所以一直没暴露；Windows 的页面源是 `http://tauri.localhost`，WebView2 会老老实实把
这个地址当 Referer 发出去，于是所有封面、头像都 403，界面上就是一片灰块。

修法：`index.html` 里加 `<meta name="referrer" content="no-referrer">` 兜底，
所有 `<img>` 再各自带 `referrerPolicy="no-referrer"`，不依赖全局设置。

### 「界面显示 B，耳朵里放的是 A，而且还按不动」

`resolve_stream` 是异步的（搜索/排行结果没有 cid，Windows 上往往还要多查一次视频详情），
在这几百毫秒里用户完全可能已经点了下一首。原来的 `loadStream()` 没有请求序号：
**旧请求晚回来也会照样写进 store**，于是 `streamUrl` 被换回上一首，界面却是新的一首；
紧接着那次 `play()` 又被换 src 打断，`AbortError` 被当成"用户暂停"写进 state，
表现就是"显示 B 放 A、播放键跳回暂停、怎么切都没用"。

现在的约束：

1. `loadStream` 带自增序号，只有最后一次的结果会被采用（`store/player.ts`）；
2. 解析开始就把 `streamUrl` 清空，`syncFromState` 在"没有地址"时**先暂停上一首**，
   绝不会让旧音频继续出声；
3. `play()` 的失败按 `AbortError`（换歌打断，忽略）/ `NotAllowedError`（等下一次用户手势补播）
   / 其它（真的失败，才把 playing 置 false 并提示）分开处理；
4. 本地代理补上了 `OPTIONS` 预检与 `Access-Control-Allow-Private-Network`，
   `<audio>` 也不再设 `crossOrigin`（纯播放不需要 CORS），避免吃 Chromium 的
   Private Network Access 规则。

## 状态管理

- `store/player.ts`：唯一可信的播放状态（队列、下标、播放中、音量、模式）。
  `lib/audio.ts` 负责把状态映射到真实的 `<audio>` 元素。
- `store/together.ts`：房间快照 + 时钟偏移。所有房间事件都收敛到 `onEvent`。
- 两者之间的桥在 `App.tsx`：订阅 together store，只在"当前曲目真的变了"时
  才让播放器重新拉流，避免加歌/聊天打断正在播的歌。
