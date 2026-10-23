//! 自建房间服务器的端到端测试：真起一个服务器 + 两个 WebSocket 客户端。
//! 覆盖「踢人」「成员进度广播」这两条最容易写错（也最容易把房主自己踢下线）的路径。

use std::time::Duration;

use bili_music_sync::client::{self, SyncEvent};
use bili_music_sync::protocol::ClientMsg;
use bili_music_sync::server::ServerState;

/// 起一个监听随机端口的房间服务器，返回 (端口, 房间号)
async fn start_server_with_room() -> (u16, String) {
    let st = ServerState { rooms: Default::default() };
    let snap = st.rooms.create("测试房间");
    let app = bili_music_sync::server::router(st);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind 失败");
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (port, snap.code)
}

/// 从事件流里等一条满足条件的事件
async fn wait_for<F>(rx: &mut tokio::sync::mpsc::UnboundedReceiver<SyncEvent>, mut ok: F) -> SyncEvent
where
    F: FnMut(&SyncEvent) -> bool,
{
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let ev = tokio::time::timeout(left, rx.recv())
            .await
            .expect("等待事件超时")
            .expect("事件流已关闭");
        if ok(&ev) {
            return ev;
        }
    }
}

#[tokio::test]
async fn kick_only_removes_the_target_member() {
    let (port, code) = start_server_with_room().await;
    let server = format!("http://127.0.0.1:{port}");

    // 先进房的是房主
    let (host, mut host_rx) = client::join(&server, &code, "房主", "").await.expect("房主进房失败");
    let host_id = match wait_for(&mut host_rx, |e| matches!(e, SyncEvent::Connected { .. })).await {
        SyncEvent::Connected { self_id, .. } => self_id,
        _ => unreachable!(),
    };
    let (_guest, mut guest_rx) = client::join(&server, &code, "客人", "").await.expect("客人进房失败");

    // 客人 id 得从房主的成员列表里拿
    let room = match wait_for(&mut host_rx, |e| match e {
        SyncEvent::Room(r) => r.members.len() >= 2,
        _ => false,
    })
    .await
    {
        SyncEvent::Room(r) => r,
        _ => unreachable!(),
    };
    let guest = room.members.iter().find(|m| m.id != host_id).expect("找不到客人").id.clone();

    // 房主踢人
    assert!(host.send(ClientMsg::Kick { member_id: guest.clone() }));

    // 被踢的人必须收到断开
    let kicked = wait_for(&mut guest_rx, |e| matches!(e, SyncEvent::Disconnected { .. })).await;
    match kicked {
        SyncEvent::Disconnected { reason } => {
            assert!(reason.contains("移出"), "断开原因不对: {reason}");
        }
        _ => unreachable!(),
    }

    // 房主自己不能被踢下线（这正是原来的 bug）
    let still_there = match wait_for(&mut host_rx, |e| match e {
        SyncEvent::Room(r) => r.members.len() == 1,
        _ => false,
    })
    .await
    {
        SyncEvent::Room(r) => r,
        _ => unreachable!(),
    };
    assert_eq!(still_there.members.len(), 1, "踢完人房间应只剩房主");
    assert_eq!(still_there.members[0].id, host_id);
    assert!(still_there.members[0].is_host, "房主标记丢了");

    // 房主连接仍然可用：还能发消息
    assert!(host.send(ClientMsg::Chat { text: "我还在".into() }));
    let chat = wait_for(&mut host_rx, |e| matches!(e, SyncEvent::Chat { .. })).await;
    match chat {
        SyncEvent::Chat { text, .. } => assert_eq!(text, "我还在"),
        _ => unreachable!(),
    }
}

#[tokio::test]
async fn progress_reaches_other_members() {
    let (port, code) = start_server_with_room().await;
    let server = format!("http://127.0.0.1:{port}");

    let (_host, mut host_rx) = client::join(&server, &code, "房主", "").await.expect("房主进房失败");
    let (guest, _guest_rx) = client::join(&server, &code, "客人", "").await.expect("客人进房失败");
    tokio::time::sleep(Duration::from_millis(300)).await;

    // 客人上报进度，房主应该收到 Members（成员列表里带上这个位置）
    assert!(guest.send(ClientMsg::Progress { position_ms: 42_000, playing: true }));
    let ev = wait_for(&mut host_rx, |e| match e {
        SyncEvent::Members(list) => list.iter().any(|m| m.position_ms == 42_000),
        _ => false,
    })
    .await;
    match ev {
        SyncEvent::Members(list) => {
            let me = list.iter().find(|m| m.position_ms == 42_000).expect("没有进度");
            assert!(me.playing, "playing 没传过来");
        }
        _ => unreachable!(),
    }
}