//! 网易云音乐：搜索 / 歌单 / 排行榜 / 播放地址 / 歌词 / 扫码登录
//! 走的是 web 端旧接口（不需要 eapi 加密），稳定且无需额外依赖。

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::http::client;
use crate::model::{Page, Track, UserInfo};
use crate::session;

const REFERER: &str = "https://music.163.com";

fn cookie_header() -> Option<String> {
    let s = session::get();
    if s.netease_cookie.trim().is_empty() {
        None
    } else {
        Some(s.netease_cookie)
    }
}

async fn request_json(req: reqwest::RequestBuilder) -> AppResult<Value> {
    let mut req = req.header("Referer", REFERER).header("Cookie", cookie_header().unwrap_or_else(|| "appver=8.9.70; os=pc".into()));
    req = req.header("Accept", "*/*");
    let resp = req.send().await?;
    let text = resp.text().await?;
    serde_json::from_str(&text).map_err(|e| {
        AppError::Message(format!("网易云返回非 JSON: {e}; 片段: {}", &text.chars().take(120).collect::<String>()))
    })
}

async fn get(url: &str) -> AppResult<Value> {
    request_json(client().get(url)).await
}

async fn post_form(url: &str, body: &str) -> AppResult<Value> {
    request_json(
        client()
            .post(url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body.to_string()),
    )
    .await
}

fn pic(url: &str, size: u32) -> String {
    if url.is_empty() {
        return String::new();
    }
    let base = if url.starts_with("http://") { url.replacen("http://", "https://", 1) } else { url.to_string() };
    format!("{base}?param={size}y{size}")
}

/// 搜索结果用的是 artists / album，歌单和详情用的是 ar / al，两种都要兼容
fn artists_of(v: &Value) -> String {
    let list = v
        .get("ar")
        .or_else(|| v.get("artists"))
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();
    list.iter()
        .filter_map(|a| a.get("name").and_then(|n| n.as_str()))
        .collect::<Vec<_>>()
        .join(" / ")
}

fn album_of(v: &Value) -> (String, String) {
    let al = v.get("al").or_else(|| v.get("album"));
    match al {
        Some(al) => (
            al.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string(),
            al.get("picUrl").and_then(|p| p.as_str()).unwrap_or("").to_string(),
        ),
        None => (String::new(), String::new()),
    }
}

fn duration_of(v: &Value) -> u64 {
    let ms = v
        .get("dt")
        .or_else(|| v.get("duration"))
        .and_then(|d| d.as_u64())
        .unwrap_or(0);
    // 搜索接口有时候给的是秒
    if ms > 0 && ms < 100_000 {
        ms * 1000
    } else {
        ms
    }
}

pub fn track_from_song(v: &Value) -> Option<Track> {
    let id = v.get("id").and_then(|i| i.as_i64())?.to_string();
    let title = v.get("name").and_then(|n| n.as_str()).unwrap_or("未命名").to_string();
    if title.is_empty() {
        return None;
    }
    let (album, pic_url) = album_of(v);
    Some(Track {
        source: "netease".into(),
        id,
        title,
        artist: artists_of(v),
        cover: pic(&pic_url, 336),
        duration_ms: duration_of(v),
        extra: None,
        album: if album.is_empty() { None } else { Some(album) },
        subtitle: None,
    })
}

pub async fn search(keyword: &str, page: u32) -> AppResult<Page<Track>> {
    let offset = (page.saturating_sub(1)) * 30;
    let body = format!(
        "s={}&type=1&offset={offset}&total=true&limit=30",
        urlencoding::encode(keyword)
    );
    let v = post_form("https://music.163.com/api/search/get/web", &body).await?;
    let songs = v.pointer("/result/songs").and_then(|s| s.as_array()).cloned().unwrap_or_default();
    let total = v.pointer("/result/songCount").and_then(|t| t.as_u64()).unwrap_or(0);
    let items: Vec<Track> = songs.iter().filter_map(track_from_song).collect();
    Ok(Page { has_more: (offset as u64 + 30) < total, items, page, total: Some(total) })
}

/// 批量查询歌曲完整信息（搜索结果缺少封面尺寸等信息时补一次）
pub async fn songs(ids: &[String]) -> AppResult<Vec<Track>> {
    let list = ids.iter().map(|id| format!("{{\"id\":{id}}}")).collect::<Vec<_>>().join(",");
    let url = format!("https://music.163.com/api/v3/song/detail?c={}", urlencoding::encode(&format!("[{list}]")));
    let v = get(&url).await?;
    let arr = v.get("songs").and_then(|s| s.as_array()).cloned().unwrap_or_default();
    Ok(arr.iter().filter_map(track_from_song).collect())
}

pub async fn song_detail(id: &str) -> AppResult<Track> {
    let url = format!("https://music.163.com/api/song/detail?ids=[{id}]");
    let v = get(&url).await?;
    v.pointer("/songs/0")
        .and_then(track_from_song)
        .ok_or_else(|| AppError::Message("歌曲不存在或已下架".into()))
}

pub async fn playlist_detail(id: &str) -> AppResult<(String, Vec<Track>)> {
    let url = format!("https://music.163.com/api/v6/playlist/detail?id={id}&n=1000&s=8");
    let v = get(&url).await?;
    if v.get("code").and_then(|c| c.as_i64()) != Some(200) {
        return Err(AppError::Message("歌单读取失败，可能需要登录".into()));
    }
    let name = v.pointer("/playlist/name").and_then(|n| n.as_str()).unwrap_or("歌单").to_string();
    let tracks = v
        .pointer("/playlist/tracks")
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default();
    Ok((name, tracks.iter().filter_map(track_from_song).collect()))
}

pub async fn toplists() -> AppResult<Vec<(String, String)>> {
    let v = get("https://music.163.com/api/toplist").await?;
    let list = v.get("list").and_then(|l| l.as_array()).cloned().unwrap_or_default();
    Ok(list
        .iter()
        .filter_map(|t| {
            let name = t.get("name").and_then(|n| n.as_str())?.to_string();
            let id = t.get("id").and_then(|i| i.as_i64())?.to_string();
            Some((name, id))
        })
        .collect())
}

pub async fn user_playlists(uid: &str) -> AppResult<Vec<(String, String, u32)>> {
    let url = format!("https://music.163.com/api/user/playlist/?uid={uid}&limit=100&offset=0&includeVideo=true");
    let v = get(&url).await?;
    let list = v.get("playlist").and_then(|l| l.as_array()).cloned().unwrap_or_default();
    Ok(list
        .iter()
        .filter_map(|p| {
            let name = p.get("name").and_then(|n| n.as_str())?.to_string();
            let id = p.get("id").and_then(|i| i.as_i64())?.to_string();
            let count = p.get("trackCount").and_then(|c| c.as_u64()).unwrap_or(0) as u32;
            Some((name, id, count))
        })
        .collect())
}

/// 播放地址（含码率/格式），拿不到时返回可读错误
pub async fn song_url(id: &str, br: u64) -> AppResult<(String, String, u64)> {
    let url = format!("https://music.163.com/api/song/enhance/player/url?ids=[{id}]&br={br}");
    let v = get(&url).await?;
    let item = v.pointer("/data/0").cloned().unwrap_or(Value::Null);
    let code = item.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
    let link = item.get("url").and_then(|u| u.as_str()).unwrap_or("").to_string();
    if link.is_empty() || code != 200 {
        let reason = match code {
            -110 => "需要登录（该歌曲为付费/会员专享）",
            404 => "版权未授权，无法播放",
            _ => "暂无可用播放地址",
        };
        return Err(AppError::Message(format!("{reason}（code {code}）")));
    }
    let ty = item.get("type").and_then(|t| t.as_str()).unwrap_or("mp3").to_string();
    let size = item.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
    let mime = if ty.eq_ignore_ascii_case("flac") { "audio/flac" } else { "audio/mpeg" };
    Ok((link, mime.to_string(), size))
}

pub async fn lyric(id: &str) -> AppResult<(String, String)> {
    let url = format!("https://music.163.com/api/song/lyric?id={id}&lv=-1&kv=-1&tv=-1");
    let v = get(&url).await?;
    let lrc = v.pointer("/lrc/lyric").and_then(|l| l.as_str()).unwrap_or("").to_string();
    let tlrc = v.pointer("/tlyric/lyric").and_then(|l| l.as_str()).unwrap_or("").to_string();
    Ok((lrc, tlrc))
}

pub async fn me() -> UserInfo {
    let Some(_) = cookie_header() else { return UserInfo::anonymous() };
    match get("https://music.163.com/api/nuser/account/get").await {
        Ok(v) => {
            let name = v.pointer("/profile/nickname").and_then(|n| n.as_str()).unwrap_or("").to_string();
            if name.is_empty() {
                return UserInfo::anonymous();
            }
            UserInfo {
                logged_in: true,
                name,
                avatar: pic(v.pointer("/profile/avatarUrl").and_then(|a| a.as_str()).unwrap_or(""), 120),
                mid: v.pointer("/profile/userId").and_then(|i| i.as_u64()).map(|i| i.to_string()).unwrap_or_default(),
            }
        }
        Err(_) => UserInfo::anonymous(),
    }
}

pub async fn qr_key() -> AppResult<String> {
    let v = get("https://music.163.com/api/login/qrcode/unikey?type=1").await?;
    v.get("unikey")
        .and_then(|k| k.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::Message("网易云二维码生成失败".into()))
}

/// 轮询扫码：800 过期 / 801 待扫描 / 802 已扫描待确认 / 803 成功
pub async fn qr_poll(key: &str) -> AppResult<(i64, String)> {
    let url = format!("https://music.163.com/api/login/qrcode/client/login?key={key}&type=1");
    let resp = client()
        .get(&url)
        .header("Referer", REFERER)
        .header("Cookie", cookie_header().unwrap_or_else(|| "appver=8.9.70; os=pc".into()))
        .send()
        .await?;
    let cookies: Vec<String> = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .collect();
    let text = resp.text().await?;
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let code = v.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
    let message = v.get("message").and_then(|m| m.as_str()).unwrap_or("").to_string();

    if code == 803 {
        let incoming = cookies.join("; ");
        let existing = session::get().netease_cookie;
        let merged = session::merge_cookie(
            &existing,
            &incoming,
            &["MUSIC_U", "__csrf", "NMTID", "MUSIC_A_T", "MUSIC_R_T"],
        );
        session::update(|s| s.netease_cookie = merged)?;
    }
    Ok((code, message))
}

pub async fn logout() -> AppResult<()> {
    session::update(|s| {
        s.netease_cookie.clear();
        s.netease_name.clear();
        s.netease_avatar.clear();
    })?;
    Ok(())
}

/// 扫码内容（前端把它渲染成二维码图片）
pub fn qr_content(key: &str) -> String {
    format!("https://music.163.com/login?codekey={key}")
}
