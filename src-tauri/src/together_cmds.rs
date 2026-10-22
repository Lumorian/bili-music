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

#[tauri::command]
pub async fn together_create(server: String, name: String) -> AppResult<Value> {
    let room = together::create(&server, &name).await?;
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
    let v = together::join(app, shared, args.server.clone(), args.code, args.name, args.avatar).await?;
    session::update(|s| s.together_server = args.server)?;
    Ok(v)
}

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
pub async fn together_set_queue(state: State<'_, SharedTogether>, queue: Vec<Track>, index: usize) -> AppResult<()> {
    let shared = state.inner().clone();
    together::push_queue(&shared, queue, index).await
}

#[tauri::command]
pub async fn together_add_track(state: State<'_, SharedTogether>, track: Track, next: Option<bool>) -> AppResult<()> {
    let shared = state.inner().clone();
    together::send(&shared, ClientMsg::AddTrack { track, next: next.unwrap_or(false) }).await
}

#[tauri::command]
pub async fn together_remove_track(state: State<'_, SharedTogether>, index: usize) -> AppResult<()> {
    let shared = state.inner().clone();
    together::send(&shared, ClientMsg::RemoveTrack { index }).await
}

#[tauri::command]
pub async fn together_play_index(state: State<'_, SharedTogether>, index: usize, position_ms: u64) -> AppResult<()> {
    let shared = state.inner().clone();
    together::send(&shared, ClientMsg::PlayIndex { index, position_ms }).await
}

#[tauri::command]
pub async fn together_set_playing(state: State<'_, SharedTogether>, playing: bool, position_ms: u64) -> AppResult<()> {
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
pub async fn together_progress(state: State<'_, SharedTogether>, position_ms: u64, playing: bool) -> AppResult<()> {
    let shared = state.inner().clone();
    together::send(&shared, ClientMsg::Progress { position_ms, playing }).await
}

#[tauri::command]
pub async fn together_transfer_host(state: State<'_, SharedTogether>, member_id: String) -> AppResult<()> {
    let shared = state.inner().clone();
    together::send(&shared, ClientMsg::TransferHost { member_id }).await
}

#[tauri::command]
pub async fn together_kick(state: State<'_, SharedTogether>, member_id: String) -> AppResult<()> {
    let shared = state.inner().clone();
    together::send(&shared, ClientMsg::Kick { member_id }).await
}

#[tauri::command]
pub async fn together_default_server() -> AppResult<String> {
    Ok(together::default_server())
}

/// 只是一次连通性测试，用于「设置」页面里的按钮
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
