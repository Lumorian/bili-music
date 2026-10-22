//! 房间状态机与协议的离线测试。

use bili_music_sync::model::Track;
use bili_music_sync::protocol::{ClientMsg, ServerMsg};
use bili_music_sync::room::{gen_code, Room, Rooms};

fn track(id: &str, dur: u64) -> Track {
    Track {
        source: "netease".into(),
        id: id.into(),
        title: format!("歌 {id}"),
        artist: "歌手".into(),
        cover: String::new(),
        duration_ms: dur,
        extra: None,
        album: None,
        subtitle: None,
    }
}

#[test]
fn room_code_is_six_digits() {
    for _ in 0..200 {
        let code = gen_code();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
        assert!(!code.starts_with('0'));
    }
}

#[test]
fn paused_position_does_not_drift() {
    let mut room = Room::new("111111".into(), "测试".into());
    room.queue.push(track("a", 200_000));
    room.set_state(30_000, false, "u1");
    // 暂停状态下无论过多久，位置都不能变（曾经这里会把流逝时间重复累加）
    std::thread::sleep(std::time::Duration::from_millis(60));
    assert_eq!(room.current_position(), 30_000);
    let snap = room.snapshot();
    assert_eq!(snap.state.position_ms, 30_000);
}

#[test]
fn playing_position_advances_and_clamps() {
    let mut room = Room::new("222222".into(), "测试".into());
    room.queue.push(track("a", 1_000));
    room.set_state(900, true, "u1");
    std::thread::sleep(std::time::Duration::from_millis(250));
    // 不能超过歌曲时长，否则进度条会跑到 100% 外面
    assert_eq!(room.current_position(), 1_000);
}

#[test]
fn next_loops_back_to_first() {
    let mut room = Room::new("333333".into(), "测试".into());
    room.queue = vec![track("a", 1000), track("b", 1000)];
    room.set_state(0, true, "u1");
    room.state.index = 1;
    assert!(room.move_index(1, "u1"));
    assert_eq!(room.state.index, 0);
    assert_eq!(room.state.position_ms, 0);
}

#[test]
fn prev_at_start_stays_at_start() {
    let mut room = Room::new("444444".into(), "测试".into());
    room.queue = vec![track("a", 1000), track("b", 1000)];
    room.state.index = 0;
    assert!(room.move_index(-1, "u1"));
    assert_eq!(room.state.index, 0);
}

#[test]
fn add_track_next_inserts_after_current() {
    let mut room = Room::new("555555".into(), "测试".into());
    room.queue = vec![track("a", 1000), track("c", 1000)];
    room.state.index = 0;
    room.add_track(track("b", 1000), true);
    assert_eq!(room.queue.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), vec!["a", "b", "c"]);
    room.add_track(track("d", 1000), false);
    assert_eq!(room.queue.last().unwrap().id, "d");
}

#[test]
fn rooms_create_and_avoid_duplicate_codes() {
    let rooms = Rooms::default();
    let snap = rooms.create("房间 A");
    assert!(rooms.exists(&snap.code));
    // 用同一个房间号建房必须失败，否则两个房间会互相覆盖
    assert!(rooms.create_with_code(&snap.code, "重复").is_none());
    assert!(rooms.create_with_code("999999", "手动号").is_some());
}

#[test]
fn protocol_roundtrip() {
    let msg = ClientMsg::AddTrack { track: track("x", 1234), next: true };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"type\":\"add_track\""));
    let back: ClientMsg = serde_json::from_str(&json).unwrap();
    matches!(back, ClientMsg::AddTrack { next: true, .. });

    let server = ServerMsg::Pong { t: 1, server_time: 2 };
    let json = serde_json::to_string(&server).unwrap();
    assert!(json.contains("\"type\":\"pong\""));
}

#[test]
fn protocol_accepts_minimal_track_json() {
    // 前端可能只传必要字段，缺省字段必须能反序列化
    let raw = r#"{"type":"add_track","track":{"source":"bilibili","id":"BV1xx","title":"t","artist":"a","cover":"","duration_ms":1000}}"#;
    let msg: ClientMsg = serde_json::from_str(raw).unwrap();
    match msg {
        ClientMsg::AddTrack { track, next } => {
            assert_eq!(track.id, "BV1xx");
            assert!(!next);
        }
        _ => panic!("解析成了错误的变体"),
    }
}
