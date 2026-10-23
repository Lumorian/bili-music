use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;
use uuid::Uuid;

use crate::model::Track;
use crate::protocol::{MemberInfo, PlaybackState, RoomSnapshot, ServerMsg};
use crate::now_ms;

/// 广播频道容量：一起听消息很少，256 足够
const CHANNEL_CAP: usize = 256;

pub struct Member {
    pub id: String,
    pub name: String,
    pub avatar: String,
    pub position_ms: u64,
    pub playing: bool,
}

pub struct Room {
    pub code: String,
    pub name: String,
    pub host_id: String,
    pub members: HashMap<String, Member>,
    pub queue: Vec<Track>,
    pub state: PlaybackState,
    pub tx: broadcast::Sender<ServerMsg>,
    /// 最后活跃时间，用于清理空房间
    pub last_active: u64,
}

impl Room {
    pub fn new(code: String, name: String) -> Self {
        let (tx, _) = broadcast::channel(CHANNEL_CAP);
        Self {
            code,
            name,
            host_id: String::new(),
            members: HashMap::new(),
            queue: Vec::new(),
            state: PlaybackState::default(),
            tx,
            last_active: now_ms(),
        }
    }

    /// 把"当前播放位置"换算到这一刻。播放中就要往前推。
    pub fn current_position(&self) -> u64 {
        let base = self.state.position_ms;
        if !self.state.playing {
            return base;
        }
        let elapsed = now_ms().saturating_sub(self.state.updated_at);
        let max = self
            .queue
            .get(self.state.index)
            .map(|t| t.duration_ms)
            .unwrap_or(u64::MAX);
        let pos = base.saturating_add(elapsed);
        if max > 0 && pos > max {
            max
        } else {
            pos
        }
    }

    /// 写入新的播放状态。
    /// 位置与时间戳必须同时更新：否则 current_position() 会把上次的位置再加一遍流逝时间，
    /// 表现为"一暂停进度条就跳到末尾"。
    pub fn set_state(&mut self, position_ms: u64, playing: bool, by: &str) {
        let now = now_ms();
        self.state.position_ms = position_ms;
        self.state.playing = playing;
        self.state.updated_at = now;
        self.state.by = by.to_string();
        self.last_active = now;
    }

    /// 成员列表（房主排最前，其余按名字稳定排序）
    pub fn member_list(&self) -> Vec<MemberInfo> {
        let mut members: Vec<MemberInfo> = self
            .members
            .values()
            .map(|m| MemberInfo {
                id: m.id.clone(),
                name: m.name.clone(),
                avatar: m.avatar.clone(),
                is_host: m.id == self.host_id,
                position_ms: m.position_ms,
                playing: m.playing,
            })
            .collect();
        members.sort_by(|a, b| b.is_host.cmp(&a.is_host).then(a.name.cmp(&b.name)));
        members
    }

    pub fn snapshot(&self) -> RoomSnapshot {
        let members = self.member_list();

        let mut state = self.state.clone();
        // 起播前 / 未起播时不当心算，避免 duration 为 0 的歌报出怪异进度
        if state.updated_at > 0 {
            state.position_ms = self.current_position();
        }
        RoomSnapshot {
            code: self.code.clone(),
            name: self.name.clone(),
            members,
            queue: self.queue.clone(),
            state,
            host_id: self.host_id.clone(),
            server_time: now_ms(),
        }
    }

    /// 广播全量房间状态
    pub fn broadcast_room(&self) {
        let _ = self.tx.send(ServerMsg::Room(self.snapshot()));
    }

    /// 只广播播放状态（进度、切歌这类高频操作）
    pub fn broadcast_state(&self) {
        let mut state = self.state.clone();
        state.position_ms = self.current_position();
        let _ = self.tx.send(ServerMsg::State { state, server_time: now_ms() });
    }

    pub fn broadcast_queue(&self) {
        let _ = self
            .tx
            .send(ServerMsg::Queue { queue: self.queue.clone(), index: self.state.index });
    }

    pub fn broadcast_chat(&self, from: &str, name: &str, text: &str) {
        let _ = self.tx.send(ServerMsg::Chat {
            from: from.to_string(),
            name: name.to_string(),
            text: text.to_string(),
            at: now_ms(),
        });
    }

    /// 下一首/上一首的公共逻辑。返回是否真的切了歌。
    pub fn move_index(&mut self, delta: isize, by: &str) -> bool {
        if self.queue.is_empty() {
            return false;
        }
        let len = self.queue.len() as isize;
        let mut next = self.state.index as isize + delta;
        if next < 0 {
            next = 0;
        }
        if next >= len {
            // 循环回第一首，一起听常见做法
            next = 0;
        }
        self.state.index = next as usize;
        self.set_state(0, true, by);
        true
    }

    pub fn add_track(&mut self, track: Track, next: bool) {
        if next && !self.queue.is_empty() {
            let at = (self.state.index + 1).min(self.queue.len());
            self.queue.insert(at, track);
        } else {
            self.queue.push(track);
        }
        self.last_active = now_ms();
    }
}

/// 应用一条客户端消息到房间状态。
/// 自建服务器和「云房间」（房主代行服务器职责）共用这一份逻辑，
/// 保证两种模式的同步语义完全一致。
pub fn apply_client_msg(room: &mut Room, member_id: &str, msg: crate::protocol::ClientMsg) {
    use crate::protocol::ClientMsg;
    match msg {
        ClientMsg::Hello { name, avatar } => {
            if let Some(m) = room.members.get_mut(member_id) {
                m.name = name;
                m.avatar = avatar;
            }
            room.broadcast_room();
        }
        ClientMsg::Sync => {
            let snap = room.snapshot();
            let _ = room.tx.send(crate::protocol::ServerMsg::Room(snap));
        }
        ClientMsg::Ping { t } => {
            let _ = room.tx.send(crate::protocol::ServerMsg::Pong { t, server_time: now_ms() });
        }
        ClientMsg::Progress { position_ms, playing } => {
            if let Some(m) = room.members.get_mut(member_id) {
                m.position_ms = position_ms;
                m.playing = playing;
            }
            room.last_active = now_ms();
            // 只广播成员列表，别把整个队列再发一遍：这是每 4 秒一次的高频包
            let members = room.member_list();
            let _ = room.tx.send(crate::protocol::ServerMsg::Members { members });
        }
        ClientMsg::SetQueue { queue, index } => {
            room.queue = queue;
            room.state.index = index.min(room.queue.len().saturating_sub(1));
            room.set_state(0, true, member_id);
            room.broadcast_queue();
            room.broadcast_state();
        }
        ClientMsg::AddTrack { track, next } => {
            let was_empty = room.queue.is_empty();
            room.add_track(track, next);
            if was_empty {
                room.state.index = 0;
                room.set_state(0, true, member_id);
                room.broadcast_state();
            }
            room.broadcast_queue();
        }
        ClientMsg::RemoveTrack { index } => {
            if index < room.queue.len() {
                room.queue.remove(index);
                if room.state.index >= room.queue.len() {
                    room.state.index = room.queue.len().saturating_sub(1);
                }
                let playing = room.state.playing;
                room.set_state(0, playing, member_id);
                room.broadcast_queue();
                room.broadcast_state();
            }
        }
        ClientMsg::PlayIndex { index, position_ms } => {
            if index < room.queue.len() {
                room.state.index = index;
                room.set_state(position_ms, true, member_id);
                room.broadcast_state();
            }
        }
        ClientMsg::SetPlaying { playing, position_ms } => {
            room.set_state(position_ms, playing, member_id);
            room.broadcast_state();
        }
        ClientMsg::Seek { position_ms } => {
            let playing = room.state.playing;
            room.set_state(position_ms, playing, member_id);
            room.broadcast_state();
        }
        ClientMsg::Next => {
            if room.move_index(1, member_id) {
                room.broadcast_state();
            }
        }
        ClientMsg::Prev => {
            if room.move_index(-1, member_id) {
                room.broadcast_state();
            }
        }
        ClientMsg::Chat { text } => {
            let name = room.members.get(member_id).map(|m| m.name.clone()).unwrap_or_default();
            room.broadcast_chat(member_id, &name, &text);
        }
        ClientMsg::Leave => {
            room.members.remove(member_id);
            if room.host_id == member_id {
                // 房主走了：交给房间里的下一个人
                room.host_id = room.members.keys().next().cloned().unwrap_or_default();
            }
            room.last_active = now_ms();
            room.broadcast_room();
        }
        ClientMsg::TransferHost { member_id: target } => {
            if room.host_id == member_id && room.members.contains_key(&target) {
                room.host_id = target;
                room.broadcast_room();
            }
        }
        ClientMsg::Kick { member_id: target } => {
            if room.host_id == member_id && target != member_id && room.members.contains_key(&target) {
                room.members.remove(&target);
                let _ = room.tx.send(crate::protocol::ServerMsg::Kicked {
                    member_id: target.clone(),
                    reason: "被房主移出房间".into(),
                });
                room.broadcast_room();
            }
        }
    }
}

/// 房间注册表：进程内的房间集合
#[derive(Clone, Default)]
pub struct Rooms {
    inner: Arc<Mutex<HashMap<String, Room>>>,
}

pub fn gen_code() -> String {
    // 6 位数字房间号，方便口头/输入法分享
    let raw = Uuid::new_v4().as_u128();
    let n = (raw % 900_000) + 100_000;
    n.to_string()
}

impl Rooms {
    pub fn create(&self, name: &str) -> RoomSnapshot {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut code = gen_code();
        while guard.contains_key(&code) {
            code = gen_code();
        }
        let room = Room::new(code.clone(), name.to_string());
        let snap = room.snapshot();
        guard.insert(code, room);
        snap
    }

    /// 用指定房间号建房；号被占了返回 None
    pub fn create_with_code(&self, code: &str, name: &str) -> Option<RoomSnapshot> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if guard.contains_key(code) {
            return None;
        }
        let room = Room::new(code.to_string(), name.to_string());
        let snap = room.snapshot();
        guard.insert(code.to_string(), room);
        Some(snap)
    }

    pub fn exists(&self, code: &str) -> bool {
        let guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.contains_key(code)
    }

    pub fn with_room<R>(&self, code: &str, f: impl FnOnce(&mut Room) -> R) -> Option<R> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.get_mut(code).map(f)
    }

    pub fn remove(&self, code: &str) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.remove(code);
    }

    /// 清理 2 小时无人活动的房间
    pub fn sweep(&self) {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let now = now_ms();
        guard.retain(|_, r| !r.members.is_empty() || now.saturating_sub(r.last_active) < 7_200_000);
    }
}
