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

## 状态管理

- `store/player.ts`：唯一可信的播放状态（队列、下标、播放中、音量、模式）。
  `lib/audio.ts` 负责把状态映射到真实的 `<audio>` 元素。
- `store/together.ts`：房间快照 + 时钟偏移。所有房间事件都收敛到 `onEvent`。
- 两者之间的桥在 `App.tsx`：订阅 together store，只在"当前曲目真的变了"时
  才让播放器重新拉流，避免加歌/聊天打断正在播的歌。
