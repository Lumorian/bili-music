//! 一起听相关的 Tauri 命令

use bili_music_sync::{ClientMsg, Track};
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, State};

use crate::error::{AppError, AppResult};
use crate::session;
use crate::together::{self, SharedTogether};

#[derive(Deserialize)]
pub struct JoinArgs {
    pub server: String,
    pub code: String,
    pub name: String,
    #[serde(default)]
    pub avatar: String,
}

#[derive(Deserialize)]
pub struct CloudArgs {
    /// 留空则用内置的公共 broker
    #[serde(default)]
    pub broker: Option<String>,
    #[serde(default)]
    pub topic_prefix: Option<String>,
    pub name: String,
    #[serde(default)]
    pub avatar: String,
    /// 加入时必填的房间号
    #[serde(default)]
    pub code: String,
}

// ---------- 云房间（不用自建服务器） ----------

/// 建一个云房间：返回 6 位房间号，把号发给朋友即可
#[tauri::command]
pub async fn together_cloud_host(
    app: AppHandle,
    state: State<'_, SharedTogether>,
    args: CloudArgs,
) -> AppResult<Value> {
    let shared = state.inner().clone();
    together::host_cloud(app, shared, args.broker, args.topic_prefix, args.name).await
}

#[tauri::command]
pub async fn together_cloud_join(
    app: AppHandle,
    state: State<'_, SharedTogether>,
    args: CloudArgs,
) -> AppResult<Value> {
    if args.code.trim().is_empty() {
        return Err(AppError::Message("请输入房间号".into()));
    }
    let shared = state.inner().clone();
    together::join_cloud(
        app,
        shared,
        args.broker,
        args.topic_prefix,
        args.code.trim().to_string(),
        args.name,
        args.avatar,
    )
    .await
}

/// 云房间的自检：只测公共 broker 能不能连上，不进房
#[tauri::command]
pub async fn together_cloud_probe(broker: Option<String>) -> AppResult<Value> {
    let cfg = crate::together_cloud::default_config();
    let url = broker.unwrap_or(cfg.broker);
    let topic = format!("{}/probe/{}", cfg.topic_prefix, crate::now_ms());
    match bili_music_sync::mqtt::MqttConn::connect(&url, "bm-probe", &topic).await {
        Ok((conn, _rx)) => {
            let ok = conn.publish(&topic, b"ping");
            Ok(json!({ "ok": ok, "broker": url }))
        }
        Err(e) => Ok(json!({ "ok": false, "broker": url, "error": e.to_string() })),
    }
}

/// 当前是不是房主
#[tauri::command]
pub async fn together_am_host(state: State<'_, SharedTogether>) -> AppResult<bool> {
    let shared = state.inner().clone();
    Ok(together::am_host(&shared).await)
}

// ---------- 自建服务器 ----------

#[tauri::command]
pub async fn together_create(server: String, name: String) -> AppResult<Value> {
    let room = together::create_server_room(&server, &name).await?;
    session::update(|s| s.together_server = server)?;
    Ok(json!({ "code": room.code, "room": room }))
}

#[tauri::command]
pub async fn together_join(
    app: AppHandle,
    state: State<'_, SharedTogether>,
    args: JoinArgs,
) -> AppResult<Value> {
    let shared = state.inner().clone();
    let v = together::join_server(app, shared, args.server.clone(), args.code, args.name, args.avatar).await?;
    session::update(|s| s.together_server = args.server)?;
    Ok(v)
}

/// 连通性测试（自建服务器）
#[tauri::command]
pub async fn together_ping(server: String) -> AppResult<Value> {
    let base = server.trim_end_matches('/');
    let resp = crate::http::client()
        .get(format!("{base}/health"))
        .send()
        .await
        .map_err(|e| AppError::Message(format!("连不上服务器: {e}")))?;
    let v: Value = resp.json().await.unwrap_or(json!({}));
    Ok(json!({ "ok": v.get("ok").and_then(|o| o.as_bool()).unwrap_or(false) }))
}

#[tauri::command]
pub async fn together_default_server() -> AppResult<String> {
    Ok(together::default_server())
}

// ---------- 房间操作（两种模式共用） ----------

#[tauri::command]
pub async fn together_snapshot(state: State<'_, SharedTogether>) -> AppResult<Value> {
    let shared = state.inner().clone();
    together::snapshot(&shared).await
}

#[tauri::command]
pub async fn together_leave(state: State<'_, SharedTogether>) -> AppResult<()> {
    let shared = state.inner().clone();
    together::leave(&shared).await
}

#[tauri::command]
pub async fn together_chat(state: State<'_, SharedTogether>, text: String) -> AppResult<()> {
    let shared = state.inner().clone();
    together::send(&shared, ClientMsg::Chat { text }).await
}

#[tauri::command]
pub async fn together_set_queue(
    state: State<'_, SharedTogether>,
    queue: Vec<Track>,
    index: usize,
) -> AppResult<()> {
    let shared = state.inner().clone();
    together::push_queue(&shared, queue, index).await
}

#[tauri::command]
pub async fn together_add_track(
    state: State<'_, SharedTogether>,
    track: Track,
    next: Option<bool>,
) -> AppResult<()> {
    let shared = state.inner().clone();
    together::send(&shared, ClientMsg::AddTrack { track, next: next.unwrap_or(false) }).await
}

#[tauri::command]
pub async fn together_remove_track(state: State<'_, SharedTogether>, index: usize) -> AppResult<()> {
    let shared = state.inner().clone();
    together::send(&shared, ClientMsg::RemoveTrack { index }).await
}

#[tauri::command]
pub async fn together_play_index(
    state: State<'_, SharedTogether>,
    index: usize,
    position_ms: u64,
) -> AppResult<()> {
    let shared = state.inner().clone();
    together::send(&shared, ClientMsg::PlayIndex { index, position_ms }).await
}

#[tauri::command]
pub async fn together_set_playing(
    state: State<'_, SharedTogether>,
    playing: bool,
    position_ms: u64,
) -> AppResult<()> {
    let shared = state.inner().clone();
    together::send(&shared, ClientMsg::SetPlaying { playing, position_ms }).await
}

#[tauri::command]
pub async fn together_seek(state: State<'_, SharedTogether>, position_ms: u64) -> AppResult<()> {
    let shared = state.inner().clone();
    together::send(&shared, ClientMsg::Seek { position_ms }).await
}

#[tauri::command]
pub async fn together_next(state: State<'_, SharedTogether>) -> AppResult<()> {
    let shared = state.inner().clone();
    together::send(&shared, ClientMsg::Next).await
}

#[tauri::command]
pub async fn together_prev(state: State<'_, SharedTogether>) -> AppResult<()> {
    let shared = state.inner().clone();
    together::send(&shared, ClientMsg::Prev).await
}

#[tauri::command]
pub async fn together_progress(
    state: State<'_, SharedTogether>,
    position_ms: u64,
    playing: bool,
) -> AppResult<()> {
    let shared = state.inner().clone();
    together::send(&shared, ClientMsg::Progress { position_ms, playing }).await
}

#[tauri::command]
pub async fn together_transfer_host(
    state: State<'_, SharedTogether>,
    member_id: String,
) -> AppResult<()> {
    let shared = state.inner().clone();
    together::send(&shared, ClientMsg::TransferHost { member_id }).await
}

#[tauri::command]
pub async fn together_kick(state: State<'_, SharedTogether>, member_id: String) -> AppResult<()> {
    let shared = state.inner().clone();
    together::send(&shared, ClientMsg::Kick { member_id }).await
}
