//! 云房间的真实联调：走 EMQX 公共 broker，需要联网。
//! 跑法：cargo test -p bili-music-sync --test cloud_live -- --nocapture --test-threads=1

use std::sync::Arc;
use std::time::Duration;

use bili_music_sync::cloud::{self, Envelope, HostLoop};
use bili_music_sync::mqtt::{CloudConfig, MqttConn};
use bili_music_sync::protocol::{ClientMsg, ServerMsg};
use bili_music_sync::room::{Member, Room};
use bili_music_sync::Track;

fn track(id: &str) -> Track {
    Track {
        source: "bilibili".into(),
        id: id.into(),
        title: format!("歌 {id}"),
        artist: "UP 主".into(),
        cover: String::new(),
        duration_ms: 200_000,
        extra: None,
        album: None,
        subtitle: None,
    }
}

/// 起一个房主：连 broker，跑状态机，返回房间号 + 本地命令口
async fn start_host() -> (String, HostLoop, Arc<MqttConn>) {
    let cfg = CloudConfig::default();
    let (conn, code, rx) = cloud::host(&cfg.broker, &cfg.topic_prefix).await.expect("建房失败");
    let topic = cloud::room_topic(&cfg.topic_prefix, &code);

    let mut room = Room::new(code.clone(), "联调房间".into());
    room.host_id = "host-self".to_string();
    room.members.insert(
        "host-self".to_string(),
        Member {
            id: "host-self".into(),
            name: "房主".into(),
            avatar: String::new(),
            position_ms: 0,
            playing: false,
        },
    );

    let conn = Arc::new(conn);
    let loop_ = cloud::spawn_host_loop(room, conn.clone(), topic, rx);
    (code, loop_, conn)
}

/// 从下行流里等出一条满足条件的信封
async fn wait_for<F>(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>, mut ok: F) -> Envelope
where
    F: FnMut(&Envelope) -> bool,
{
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let bytes = tokio::time::timeout(left, rx.recv())
            .await
            .expect("等消息超时")
            .expect("下行流关闭");
        if let Some(env) = cloud::decode(&bytes) {
            if ok(&env) {
                return env;
            }
        }
    }
}

#[tokio::test]
async fn mqtt_broker_reachable() {
    let cfg = CloudConfig::default();
    let topic = cloud::room_topic(&cfg.topic_prefix, "ping");
    let (conn, _rx) = MqttConn::connect(&cfg.broker, "bm-ping", &topic)
        .await
        .expect("连公共 broker 失败");
    assert!(conn.publish(&topic, b"hello"));
    println!("公共 broker 可用: {}", cfg.broker);
}

#[tokio::test]
async fn guest_joins_and_gets_welcome() {
    let (code, mut host_loop, _conn) = start_host().await;
    let cfg = CloudConfig::default();

    let (guest_conn, guest_id, mut guest_rx) =
        cloud::join(&cfg.broker, &cfg.topic_prefix, &code, "小明", "").await.expect("进房失败");
    println!("房间 {code}，我是 {guest_id}");

    // 房主应该收到 Hello 并把 Welcome 单发给这个新成员
    let env = wait_for(&mut guest_rx, |e| matches!(e, Envelope::Direct { .. })).await;
    match env {
        Envelope::Direct { to, msg } => {
            assert_eq!(to, guest_id, "Welcome 应该只发给新来的人");
            match msg {
                ServerMsg::Welcome { self_id, room } => {
                    assert_eq!(self_id, guest_id);
                    assert!(room.members.len() >= 2, "房主 + 新成员");
                    println!("收到 Welcome，成员数 {}", room.members.len());
                }
                other => panic!("期望 Welcome，收到 {other:?}"),
            }
        }
        _ => unreachable!(),
    }

    // 本地投递一条操作，确认房主状态机吃得到
    let _ = host_loop.cmd_tx.send(("host-self".into(), ClientMsg::AddTrack { track: track("A"), next: false }));
    let msg = wait_for_state(&mut host_loop.out_rx, |m| matches!(m, ServerMsg::Queue { .. })).await;
    if let ServerMsg::Queue { queue, .. } = msg {
        assert_eq!(queue.len(), 1);
    }

    let _ = guest_conn;
}

async fn wait_for_state<F>(rx: &mut tokio::sync::mpsc::UnboundedReceiver<ServerMsg>, mut ok: F) -> ServerMsg
where
    F: FnMut(&ServerMsg) -> bool,
{
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let msg = tokio::time::timeout(left, rx.recv())
            .await
            .expect("等 ServerMsg 超时")
            .expect("通道关闭");
        if ok(&msg) {
            return msg;
        }
    }
}

#[tokio::test]
async fn guest_operation_broadcasts_to_everyone() {
    let (code, host_loop, _conn) = start_host().await;
    let cfg = CloudConfig::default();

    let (guest_conn, guest_id, _guest_rx) =
        cloud::join(&cfg.broker, &cfg.topic_prefix, &code, "小红", "").await.expect("进房失败");
    tokio::time::sleep(Duration::from_millis(800)).await;

    // 第二个观察者：只收广播
    let topic = cloud::room_topic(&cfg.topic_prefix, &code);
    let (_watcher, mut watcher_rx) = MqttConn::connect(&cfg.broker, "bm-watch", &topic)
        .await
        .expect("观察者连接失败");
    tokio::time::sleep(Duration::from_millis(700)).await;

    // 客人加歌
    guest_conn.publish(&topic, &cloud::client_envelope(&guest_id, ClientMsg::AddTrack { track: track("G1"), next: false }));

    // 房主处理后应广播出 Queue
    let msg = wait_for_state(&mut tokio::sync::mpsc::UnboundedReceiver::from(host_loop.out_rx), |m| {
        matches!(m, ServerMsg::Queue { .. })
    })
    .await;
    if let ServerMsg::Queue { queue, .. } = msg {
        assert_eq!(queue.len(), 1, "客人加的歌应该进队列");
        assert_eq!(queue[0].id, "G1");
        println!("房主状态机已接纳客人的操作");
    }

    // 观察者也应该从 broker 上收到广播
    let env = wait_for(&mut watcher_rx, |e| matches!(e, Envelope::Server { msg: ServerMsg::Queue { .. } })).await;
    if let Envelope::Server { msg: ServerMsg::Queue { queue, .. } } = env {
        assert_eq!(queue.len(), 1);
        println!("公共 broker 广播验证通过");
    }
}
