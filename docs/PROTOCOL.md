# 一起听协议

传输：WebSocket，每条消息是一个 JSON 文本帧，用 `type` 字段区分类型
（Rust 侧 `#[serde(tag = "type", rename_all = "snake_case")]`）。

## 连接

```
ws://<server>/ws/<房间号>?name=<昵称>&avatar=<头像URL>
```

连上后服务器立刻推送 `welcome`，含 `self_id` 与完整房间快照；
随后该连接会收到房间里所有广播。

## 客户端 → 服务器

| type | 字段 | 说明 |
| --- | --- | --- |
| `hello` | `name`, `avatar?` | 进房后自报昵称（连接参数已有，这个是二次确认） |
| `sync` | — | 请求一次全量房间快照 |
| `ping` | `t` | 心跳，服务器回 `pong` 带回 `server_time` |
| `progress` | `position_ms`, `playing` | 上报自己的播放进度（只用于成员小进度条） |
| `set_queue` | `queue[]`, `index` | 整体替换队列 |
| `add_track` | `track`, `next?` | 加歌，`next=true` 插到当前曲目之后 |
| `remove_track` | `index` | 移除队列中的一首 |
| `play_index` | `index`, `position_ms` | 播放指定下标 |
| `set_playing` | `playing`, `position_ms` | 播放 / 暂停 |
| `seek` | `position_ms` | 拖动进度 |
| `next` / `prev` | — | 上一首 / 下一首 |
| `chat` | `text` | 房间聊天 |
| `transfer_host` | `member_id` | 移交房主（仅房主） |
| `kick` | `member_id` | 踢人（仅房主） |

## 服务器 → 客户端

| type | 字段 | 说明 |
| --- | --- | --- |
| `welcome` | `self_id`, `room` | 连接成功，附完整快照 |
| `room` | `room` | 房间全量状态（成员变化、房主变化等） |
| `state` | `state`, `server_time` | 播放状态变化（高频，只带必要字段） |
| `queue` | `queue[]`, `index` | 队列变化 |
| `chat` | `from`, `name`, `text`, `at` | 聊天消息 |
| `pong` | `t`, `server_time` | 心跳回应 |
| `kicked` | `reason` | 被移出房间 |
| `error` | `message` | 错误提示 |

## 时间对齐

`playback_state` 里有三个字段要一起看：

```json
{ "index": 2, "position_ms": 42000, "playing": true, "updated_at": 1792648351863, "by": "<member id>" }
```

- `position_ms` 是 **`updated_at` 那一刻**的位置，不是"现在"的位置。
- 客户端算期望位置：`position + (本地时间 + clockOffset - updated_at)`，
  其中 `clockOffset = server_time - 本地时间`（每次收到消息都更新）。
- 实际播放位置与期望偏差 **> 1200ms** 才强制 seek，避免每 4 秒抖动一次。
- 本地手动拖动进度后 2.5 秒内不做纠偏（`guardSeek`），否则会和服务器回声打架。

## 房间号

6 位数字，服务器随机生成且保证不重复；`POST /api/rooms` 也可以指定 `code`
（被占用会返回 409）。成员 id 形如 `<6位数字>-<6位十六进制>`，房间内唯一。
