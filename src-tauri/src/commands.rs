//! 暴露给前端的 Tauri 命令，前端通过 invoke() 调用。

use serde::Serialize;
use serde_json::{json, Value};

use crate::bilibili;
use crate::error::{AppError, AppResult};
use crate::model::{Page, StreamInfo, Track, UserInfo};
use crate::netease;
use crate::proxy;
use crate::session;

#[derive(Serialize)]
pub struct TrackDetail {
    pub track: Track,
    pub desc: String,
    /// (cid, 分P标题, 时长秒)
    pub pages: Vec<(u64, String, u64)>,
}

#[derive(Serialize)]
pub struct QrPayload {
    pub key: String,
    /// 二维码里要放的字符串，前端用 qrcode 库画出来
    pub content: String,
}

/// 统一解析播放地址：不管来自哪个平台，都变成可给 <audio> 用的本地代理地址
pub async fn resolve_stream_inner(track: &Track) -> AppResult<StreamInfo> {
    let port = proxy::state().port;
    if port == 0 {
        return Err(AppError::Message("本地音频代理未启动".into()));
    }

    match track.source.as_str() {
        "bilibili" => {
            let cid = match &track.extra {
                Some(c) => c.parse::<u64>().unwrap_or(0),
                None => 0,
            };
            let (bvid, cid) = if cid > 0 {
                (track.id.clone(), cid)
            } else {
                let d = bilibili::video_detail(&track.id).await?;
                (d.track.id.clone(), d.cid)
            };
            let audio = bilibili::resolve_audio(&bvid, cid, None).await?;
            let (_, url) = proxy::state().register(
                audio.url.clone(),
                audio.backups.clone(),
                "https://www.bilibili.com".into(),
                audio.mime.clone(),
            );
            let _ = port;
            Ok(StreamInfo {
                stream_url: url,
                url: audio.url,
                mime: audio.mime,
                expires_at: crate::now_ms() + 7_200_000,
            })
        }
        "netease" => {
            let (link, mime, _size) = netease::song_url(&track.id, 320_000).await?;
            let (_, url) = proxy::state().register(
                link.clone(),
                Vec::new(),
                "https://music.163.com".into(),
                mime.clone(),
            );
            Ok(StreamInfo {
                stream_url: url,
                url: link,
                mime,
                expires_at: crate::now_ms() + 7_200_000,
            })
        }
        other => Err(AppError::Message(format!("未知来源: {other}"))),
    }
}

#[tauri::command]
pub async fn resolve_stream(track: Track) -> AppResult<StreamInfo> {
    resolve_stream_inner(&track).await
}

// ---------- B 站 ----------

#[tauri::command]
pub async fn bili_search(keyword: String, page: Option<u32>) -> AppResult<Page<Track>> {
    bilibili::search(&keyword, page.unwrap_or(1)).await
}

#[tauri::command]
pub async fn bili_ranking(rid: Option<u32>, page: Option<u32>) -> AppResult<Page<Track>> {
    bilibili::ranking(rid.unwrap_or(3), page.unwrap_or(1)).await
}

#[tauri::command]
pub async fn bili_related(bvid: String) -> AppResult<Vec<Track>> {
    bilibili::related(&bvid).await
}

#[tauri::command]
pub async fn bili_detail(bvid: String) -> AppResult<TrackDetail> {
    let d = bilibili::video_detail(&bvid).await?;
    Ok(TrackDetail { track: d.track, desc: d.desc, pages: d.pages })
}

#[tauri::command]
pub async fn bili_regions() -> AppResult<Vec<Value>> {
    Ok(bilibili::music_regions()
        .into_iter()
        .map(|(name, id)| json!({ "name": name, "id": id }))
        .collect())
}

#[tauri::command]
pub async fn bili_toview() -> AppResult<Vec<Track>> {
    bilibili::toview().await
}

#[tauri::command]
pub async fn bili_fav_folders() -> AppResult<Value> {
    let me = bilibili::me().await;
    if !me.logged_in {
        return Ok(json!([]));
    }
    let v = bilibili::fav_folders(&me.mid).await?;
    Ok(v.pointer("/data/list").cloned().unwrap_or_else(|| json!([])))
}

#[tauri::command]
pub async fn bili_fav_list(media_id: u64, page: Option<u32>) -> AppResult<Page<Track>> {
    bilibili::fav_list(media_id, page.unwrap_or(1)).await
}

#[tauri::command]
pub async fn bili_me() -> AppResult<UserInfo> {
    Ok(bilibili::me().await)
}

#[tauri::command]
pub async fn bili_qr() -> AppResult<QrPayload> {
    let qr = bilibili::qr_generate().await?;
    Ok(QrPayload { content: qr.url, key: qr.key })
}

/// 返回 (状态码, 说明)：0 成功 / 86101 未扫描 / 86090 已扫描待确认 / 86038 已过期
#[tauri::command]
pub async fn bili_qr_poll(key: String) -> AppResult<Value> {
    let (code, message) = bilibili::qr_poll(&key).await?;
    if code == 0 {
        let me = bilibili::me().await;
        session::update(|s| {
            s.bilibili_name = me.name.clone();
            s.bilibili_avatar = me.avatar.clone();
        })?;
    }
    Ok(json!({ "code": code, "message": message }))
}

#[tauri::command]
pub async fn bili_logout() -> AppResult<()> {
    bilibili::logout().await
}

// ---------- 网易云 ----------

#[tauri::command]
pub async fn ne_search(keyword: String, page: Option<u32>) -> AppResult<Page<Track>> {
    netease::search(&keyword, page.unwrap_or(1)).await
}

#[tauri::command]
pub async fn ne_song(id: String) -> AppResult<Track> {
    netease::song_detail(&id).await
}

#[tauri::command]
pub async fn ne_lyric(id: String) -> AppResult<Value> {
    let (lrc, tlrc) = netease::lyric(&id).await?;
    Ok(json!({ "lrc": lrc, "translation": tlrc }))
}

#[tauri::command]
pub async fn ne_toplists() -> AppResult<Vec<Value>> {
    let list = netease::toplists().await?;
    Ok(list.into_iter().map(|(name, id)| json!({ "name": name, "id": id })).collect())
}

#[tauri::command]
pub async fn ne_playlist(id: String) -> AppResult<Value> {
    let (name, tracks) = netease::playlist_detail(&id).await?;
    Ok(json!({ "name": name, "tracks": tracks }))
}

#[tauri::command]
pub async fn ne_my_playlists() -> AppResult<Vec<Value>> {
    let me = netease::me().await;
    if !me.logged_in || me.mid.is_empty() {
        return Ok(Vec::new());
    }
    let list = netease::user_playlists(&me.mid).await?;
    Ok(list
        .into_iter()
        .map(|(name, id, count)| json!({ "name": name, "id": id, "count": count }))
        .collect())
}

#[tauri::command]
pub async fn ne_me() -> AppResult<UserInfo> {
    Ok(netease::me().await)
}

#[tauri::command]
pub async fn ne_qr() -> AppResult<QrPayload> {
    let key = netease::qr_key().await?;
    let content = netease::qr_content(&key);
    Ok(QrPayload { content, key })
}

/// 返回 (状态码, 说明)：800 过期 / 801 待扫描 / 802 已扫描待确认 / 803 成功
#[tauri::command]
pub async fn ne_qr_poll(key: String) -> AppResult<Value> {
    let (code, message) = netease::qr_poll(&key).await?;
    if code == 803 {
        let me = netease::me().await;
        session::update(|s| {
            s.netease_name = me.name.clone();
            s.netease_avatar = me.avatar.clone();
        })?;
    }
    Ok(json!({ "code": code, "message": message }))
}

#[tauri::command]
pub async fn ne_logout() -> AppResult<()> {
    netease::logout().await
}

// ---------- 会话 / 设置 ----------

#[tauri::command]
pub async fn get_settings() -> AppResult<Value> {
    let s = session::get();
    Ok(json!({
        "togetherServer": s.together_server,
        "nickname": s.nickname,
        "bilibili": { "name": s.bilibili_name, "avatar": s.bilibili_avatar, "loggedIn": !s.bilibili_cookie.is_empty() && !s.bilibili_name.is_empty() },
        "netease": { "name": s.netease_name, "avatar": s.netease_avatar, "loggedIn": !s.netease_cookie.is_empty() && !s.netease_name.is_empty() },
    }))
}

#[tauri::command]
pub async fn save_settings(together_server: Option<String>, nickname: Option<String>) -> AppResult<()> {
    session::update(|s| {
        if let Some(v) = together_server {
            s.together_server = v;
        }
        if let Some(n) = nickname {
            s.nickname = n;
        }
    })?;
    Ok(())
}

#[tauri::command]
pub async fn app_info() -> AppResult<Value> {
    Ok(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "proxyPort": proxy::state().port,
        "platform": std::env::consts::OS,
    }))
}
