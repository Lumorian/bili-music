//! B 站接口：搜索 / 排行 / 视频详情 / 音频流 / 扫码登录
//! 全部走 wbi 签名，UA 必须带 Referer，否则风控会返回 -352 / -403。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::http::client;
use crate::model::{Page, Track, UserInfo};
use crate::session;

const REFERER: &str = "https://www.bilibili.com";

/// wbi 混淆表
const MIXIN_KEY_ENC_TAB: [usize; 64] = [
    46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29,
    28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51, 30, 4, 22,
    25, 54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
];

struct WbiCache {
    mixin_key: String,
    at: Instant,
}

static WBI: Mutex<Option<WbiCache>> = Mutex::new(None);

/// BUVID 设备指纹缓存（进程内）
static DEVICE: Mutex<Option<(String, String)>> = Mutex::new(None);
/// 首屏会并发打好几个接口，用一个异步锁保证设备指纹只申请一次
static DEVICE_FETCH: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// 拉一次 buvid3 / buvid4。
/// B 站现在对没有设备指纹的请求（尤其是 ranking）会直接返回 -352 风控，
/// 而扫码登录成功后拿到的 cookie 里也不一定带 buvid，所以自己先要一份。
async fn fetch_device_ids() -> AppResult<(String, String)> {
    let resp = client()
        .get("https://api.bilibili.com/x/frontend/finger/spi")
        .header("Referer", REFERER)
        .header("Accept", "application/json, text/plain, */*")
        .send()
        .await?;
    let v: Value = serde_json::from_str(&resp.text().await?)?;
    let b3 = v.pointer("/data/b_3").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let b4 = v.pointer("/data/b_4").and_then(|x| x.as_str()).unwrap_or("").to_string();
    if b3.is_empty() {
        return Err(AppError::Message("无法获取 B 站设备指纹".into()));
    }
    Ok((b3, b4))
}

/// 确保手里有 buvid3 / buvid4（内存 → 会话文件 → 重新申请）
async fn device_ids() -> Option<(String, String)> {
    if let Ok(guard) = DEVICE.lock() {
        if let Some(v) = guard.as_ref() {
            return Some(v.clone());
        }
    }
    // 串行化：首屏并发打好几个接口时，设备指纹只申请一次
    let _guard = DEVICE_FETCH.lock().await;
    if let Ok(guard) = DEVICE.lock() {
        if let Some(v) = guard.as_ref() {
            return Some(v.clone());
        }
    }
    let saved = session::get();
    if !saved.buvid3.is_empty() {
        let v = (saved.buvid3.clone(), saved.buvid4.clone());
        if let Ok(mut guard) = DEVICE.lock() {
            *guard = Some(v.clone());
        }
        return Some(v);
    }

    match fetch_device_ids().await {
        Ok(v) => {
            let cloned = v.clone();
            let _ = session::update(|s| {
                s.buvid3 = cloned.0.clone();
                s.buvid4 = cloned.1.clone();
            });
            if let Ok(mut guard) = DEVICE.lock() {
                *guard = Some(v.clone());
            }
            log::info!("已获取 B 站设备指纹 buvid3={}…", &v.0.chars().take(8).collect::<String>());
            Some(v)
        }
        Err(e) => {
            log::warn!("获取设备指纹失败: {e}");
            None
        }
    }
}

/// 组装请求用的 Cookie：登录态 + 设备指纹
async fn cookie_header() -> Option<String> {
    let s = session::get();
    let mut parts: Vec<String> = Vec::new();
    if !s.bilibili_cookie.trim().is_empty() {
        parts.push(s.bilibili_cookie.trim().to_string());
    }
    if let Some((b3, b4)) = device_ids().await {
        if !s.bilibili_cookie.contains("buvid3=") {
            parts.push(format!("buvid3={b3}"));
        }
        if !b4.is_empty() && !s.bilibili_cookie.contains("buvid4=") {
            parts.push(format!("buvid4={b4}"));
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("; "))
    }
}

async fn get_json(url: &str) -> AppResult<Value> {
    let mut req = client()
        .get(url)
        .header("Referer", REFERER)
        .header("Origin", "https://www.bilibili.com")
        .header("Accept", "application/json, text/plain, */*");
    if let Some(c) = cookie_header().await {
        req = req.header("Cookie", c);
    }
    let resp = req.send().await?;
    let text = resp.text().await?;
    serde_json::from_str(&text).map_err(|e| AppError::Message(format!("B 站返回非 JSON: {e}; 片段: {}", &text.chars().take(160).collect::<String>())))
}

fn code_of(v: &Value) -> i64 {
    v.get("code").and_then(|c| c.as_i64()).unwrap_or(-1)
}

fn msg_of(v: &Value) -> String {
    v.get("message")
        .and_then(|m| m.as_str())
        .unwrap_or("未知错误")
        .to_string()
}

/// 拉取 wbi 签名密钥（缓存 30 分钟）
async fn mixin_key() -> AppResult<String> {
    if let Ok(guard) = WBI.lock() {
        if let Some(c) = guard.as_ref() {
            if c.at.elapsed() < Duration::from_secs(1800) {
                return Ok(c.mixin_key.clone());
            }
        }
    }

    let v = get_json("https://api.bilibili.com/x/web-interface/nav").await?;
    let img = v
        .pointer("/data/wbi_img/img_url")
        .and_then(|x| x.as_str())
        .ok_or_else(|| AppError::Message("无法获取 wbi 签名密钥".into()))?
        .to_string();
    let sub = v
        .pointer("/data/wbi_img/sub_url")
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string();

    let stem = |u: &str| -> String {
        u.rsplit('/')
            .next()
            .unwrap_or("")
            .split('.')
            .next()
            .unwrap_or("")
            .to_string()
    };
    let raw = format!("{}{}", stem(&img), stem(&sub));
    let key: String = MIXIN_KEY_ENC_TAB
        .iter()
        .map(|&i| raw.chars().nth(i).unwrap_or('0'))
        .collect::<String>()
        .chars()
        .take(32)
        .collect();

    if let Ok(mut guard) = WBI.lock() {
        *guard = Some(WbiCache { mixin_key: key.clone(), at: Instant::now() });
    }
    Ok(key)
}

/// 对参数做 wbi 签名，返回 query string
async fn signed_query(params: &[(&str, String)]) -> AppResult<String> {
    let key = mixin_key().await?;
    let mut map: Vec<(String, String)> = params
        .iter()
        .map(|(k, v)| {
            let cleaned: String = v.chars().filter(|c| !matches!(c, '!' | '\'' | '(' | ')' | '*')).collect();
            (k.to_string(), cleaned)
        })
        .collect();
    map.push((
        "wts".to_string(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .to_string(),
    ));
    map.sort_by(|a, b| a.0.cmp(&b.0));

    let query = map
        .iter()
        .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
        .collect::<Vec<_>>()
        .join("&");

    let sign = md5_hex(&format!("{query}{key}"));
    Ok(format!("{query}&w_rid={sign}"))
}

pub fn md5_hex(input: &str) -> String {
    use md5::{Digest, Md5};
    let mut hasher = Md5::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

async fn wbi_get(path_with_query: &str, params: &[(&str, String)]) -> AppResult<Value> {
    // -352 是 B 站最烦人的风控码：签名本身没错，多数时候是缺设备指纹、
    // 或者同一个 wbi 密钥用得久了。这里按「换签名 → 换密钥 → 换设备指纹 → 退避」
    // 逐步升级，最多试 4 轮，尽量不让用户看见报错。
    let mut last_code = -1;
    let mut last_msg = String::new();
    for attempt in 0..4u32 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(400 * u64::from(attempt))).await;
        }
        let qs = signed_query(params).await?;
        let url = format!("https://api.bilibili.com{path_with_query}?{qs}");
        let v = get_json(&url).await?;
        let code = code_of(&v);
        if code == 0 {
            return Ok(v);
        }
        last_code = code;
        last_msg = msg_of(&v);
        match code {
            // 签名失效：丢掉缓存的 mixin key
            -403 => {
                if let Ok(mut g) = WBI.lock() {
                    *g = None;
                }
            }
            // 风控：先换密钥，再换设备指纹
            -352 => {
                if let Ok(mut g) = WBI.lock() {
                    *g = None;
                }
                if attempt >= 1 {
                    if let Ok(mut g) = DEVICE.lock() {
                        *g = None;
                    }
                    let _ = session::update(|s| {
                        s.buvid3.clear();
                        s.buvid4.clear();
                    });
                    let _ = device_ids().await;
                }
            }
            _ => {
                return Err(AppError::Message(format!("B 站接口错误({code}): {last_msg}")));
            }
        }
        log::warn!("B 站接口 {path_with_query} 返回 {code}（第 {} 次尝试），准备重试", attempt + 1);
    }
    Err(AppError::Message(format!("B 站接口被风控({last_code}): {last_msg}")))
}

fn cover(url: &str) -> String {
    if url.is_empty() {
        return String::new();
    }
    let u = if url.starts_with("//") {
        format!("https:{url}")
    } else if url.starts_with("http://") {
        url.replacen("http://", "https://", 1)
    } else {
        url.to_string()
    };
    // 走 B 站图床的裁剪参数，省流量
    format!("{u}@336w_336h_1c.webp")
}

fn play_count(n: u64) -> String {
    if n >= 100_000_000 {
        format!("{:.1}亿播放", n as f64 / 100_000_000.0)
    } else if n >= 10_000 {
        format!("{:.1}万播放", n as f64 / 10_000.0)
    } else {
        format!("{n}播放")
    }
}

/// 视频搜索结果里既可能是 video 也可能混入其他类型
fn track_from_search_item(item: &Value) -> Option<Track> {
    let bvid = item.get("bvid").and_then(|v| v.as_str())?.to_string();
    let title = strip_em(item.get("title").and_then(|v| v.as_str()).unwrap_or("未命名"));
    let author = item.get("author").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let duration = item.get("duration").and_then(|v| v.as_str()).unwrap_or("0:00");
    Some(Track {
        source: "bilibili".into(),
        id: bvid,
        title,
        artist: author,
        cover: cover(item.get("pic").and_then(|v| v.as_str()).unwrap_or("")),
        duration_ms: parse_duration(duration),
        // 搜索结果只给 bvid，cid 留到解析音轨时再查
        extra: None,
        album: None,
        subtitle: item.get("play").and_then(|v| v.as_u64()).map(play_count),
    })
}

fn strip_em(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&quot;", "\"").replace("&amp;", "&").replace("&#39;", "'").replace("&lt;", "<").replace("&gt;", ">")
}

fn parse_duration(s: &str) -> u64 {
    let parts: Vec<u64> = s.split(':').filter_map(|p| p.parse::<u64>().ok()).collect();
    match parts.len() {
        3 => parts[0] * 3_600_000 + parts[1] * 60_000 + parts[2] * 1000,
        2 => parts[0] * 60_000 + parts[1] * 1000,
        1 => parts[0] * 1000,
        _ => 0,
    }
}

#[derive(Deserialize)]
struct ViewPage {
    cid: u64,
    #[serde(default)]
    part: String,
    #[serde(default)]
    duration: u64,
}

#[derive(Deserialize)]
struct ViewData {
    bvid: String,
    title: String,
    #[serde(default)]
    pic: String,
    #[serde(default)]
    desc: String,
    #[serde(default)]
    duration: u64,
    #[serde(default)]
    videos: u32,
    owner: ViewOwner,
    #[serde(default)]
    pages: Vec<ViewPage>,
    #[serde(default)]
    stat: ViewStat,
}

#[derive(Deserialize)]
struct ViewOwner {
    name: String,
}

#[derive(Deserialize, Default)]
struct ViewStat {
    #[serde(default)]
    view: u64,
}

pub struct VideoDetail {
    pub track: Track,
    pub cid: u64,
    pub desc: String,
    pub pages: Vec<(u64, String, u64)>,
    pub videos: u32,
}

pub async fn video_detail(bvid: &str) -> AppResult<VideoDetail> {
    let v = get_json(&format!("https://api.bilibili.com/x/web-interface/view?bvid={bvid}")).await?;
    if code_of(&v) != 0 {
        return Err(AppError::Message(format!("视频信息获取失败({}): {}", code_of(&v), msg_of(&v))));
    }
    let data: ViewData = serde_json::from_value(v["data"].clone())?;
    let pages: Vec<(u64, String, u64)> = data
        .pages
        .iter()
        .map(|p| (p.cid, p.part.clone(), p.duration))
        .collect();
    let cid = pages.first().map(|p| p.0).unwrap_or(0);
    let track = Track {
        source: "bilibili".into(),
        id: data.bvid.clone(),
        title: data.title.clone(),
        artist: data.owner.name.clone(),
        cover: cover(&data.pic),
        duration_ms: if data.duration > 0 { data.duration * 1000 } else { pages.first().map(|p| p.2 * 1000).unwrap_or(0) },
        extra: Some(cid.to_string()),
        album: None,
        subtitle: Some(play_count(data.stat.view)),
    };
    Ok(VideoDetail { track, cid, desc: data.desc, pages, videos: data.videos })
}

/// 搜索（默认只搜视频，B 站没有"歌曲"维度）
pub async fn search(keyword: &str, page: u32) -> AppResult<Page<Track>> {
    let v = wbi_get(
        "/x/web-interface/wbi/search/type",
        &[
            ("search_type", "video".to_string()),
            ("keyword", keyword.to_string()),
            ("page", page.to_string()),
            ("page_size", "30".to_string()),
            ("order", "totalrank".to_string()),
        ],
    )
    .await?;

    let result = v.pointer("/data/result").and_then(|r| r.as_array()).cloned().unwrap_or_default();
    let items: Vec<Track> = result.iter().filter_map(track_from_search_item).collect();
    let num_results = v.pointer("/data/numResults").and_then(|n| n.as_u64()).unwrap_or(0);
    let pages = v.pointer("/data/numPages").and_then(|n| n.as_u64()).unwrap_or(0) as u32;
    Ok(Page { has_more: page < pages, items, page, total: Some(num_results) })
}

/// 分区排行（rid=3 是音乐区），电台/音乐合集的富矿。
/// ranking/v2 是最容易被 -352 风控的一个接口，实在拿不到就退回老的
/// /x/web-interface/ranking/region（数据量少一点，但几乎不风控）。
pub async fn ranking(rid: u32, page: u32) -> AppResult<Page<Track>> {
    if page > 1 {
        // 老接口没有分页，翻页时继续用 v2
        return ranking_v2(rid, page).await;
    }
    match ranking_v2(rid, page).await {
        Ok(v) => Ok(v),
        Err(e) => match ranking_region(rid).await {
            Ok(v) if !v.items.is_empty() => {
                log::warn!("ranking/v2 失败（{e}），已退回 ranking/region");
                Ok(v)
            }
            _ => Err(e),
        },
    }
}

async fn ranking_v2(rid: u32, page: u32) -> AppResult<Page<Track>> {
    let v = wbi_get(
        "/x/web-interface/ranking/v2",
        &[("rid", rid.to_string()), ("type", "all".to_string()), ("ps", "30".to_string()), ("pn", page.to_string())],
    )
    .await?;
    let list = v.pointer("/data/list").and_then(|l| l.as_array()).cloned().unwrap_or_default();
    let items: Vec<Track> = list
        .iter()
        .filter_map(|item| {
            let bvid = item.get("bvid").and_then(|x| x.as_str())?.to_string();
            Some(Track {
                source: "bilibili".into(),
                id: bvid,
                title: item.get("title").and_then(|x| x.as_str()).unwrap_or("未命名").to_string(),
                artist: item.pointer("/owner/name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                cover: cover(item.get("pic").and_then(|x| x.as_str()).unwrap_or("")),
                duration_ms: item.get("duration").and_then(|x| x.as_u64()).unwrap_or(0) * 1000,
                extra: None,
                album: None,
                subtitle: item.pointer("/stat/view").and_then(|x| x.as_u64()).map(play_count),
            })
        })
        .collect();
    Ok(Page { has_more: page < 4, items, page, total: None })
}

/// 备用排行接口：字段风格老一些，但风控宽松
async fn ranking_region(rid: u32) -> AppResult<Page<Track>> {
    let url = format!("https://api.bilibili.com/x/web-interface/ranking/region?rid={rid}&day=7");
    let v = get_json(&url).await?;
    if code_of(&v) != 0 {
        return Err(AppError::Message(format!("排行榜获取失败({}): {}", code_of(&v), msg_of(&v))));
    }
    let list = v.get("data").and_then(|l| l.as_array()).cloned().unwrap_or_default();
    let items: Vec<Track> = list
        .iter()
        .filter_map(|item| {
            let bvid = item.get("bvid").and_then(|x| x.as_str())?.to_string();
            Some(Track {
                source: "bilibili".into(),
                id: bvid,
                title: strip_em(item.get("title").and_then(|x| x.as_str()).unwrap_or("未命名")),
                artist: item.get("author").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                cover: cover(item.get("pic").and_then(|x| x.as_str()).unwrap_or("")),
                duration_ms: parse_duration(item.get("duration").and_then(|x| x.as_str()).unwrap_or("0:00")),
                extra: None,
                album: None,
                subtitle: item.get("play").and_then(|x| x.as_u64()).map(play_count),
            })
        })
        .collect();
    Ok(Page { has_more: false, items, page: 1, total: None })
}

/// 相关推荐：用来做"自动接歌"，听完一首自动找下一首
pub async fn related(bvid: &str) -> AppResult<Vec<Track>> {
    let v = get_json(&format!("https://api.bilibili.com/x/web-interface/archive/related?bvid={bvid}")).await?;
    if code_of(&v) != 0 {
        return Ok(Vec::new());
    }
    let list = v.get("data").and_then(|d| d.as_array()).cloned().unwrap_or_default();
    Ok(list
        .iter()
        .filter_map(|item| {
            let bvid = item.get("bvid").and_then(|x| x.as_str())?.to_string();
            Some(Track {
                source: "bilibili".into(),
                id: bvid,
                title: item.get("title").and_then(|x| x.as_str()).unwrap_or("未命名").to_string(),
                artist: item.pointer("/owner/name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                cover: cover(item.get("pic").and_then(|x| x.as_str()).unwrap_or("")),
                duration_ms: item.get("duration").and_then(|x| x.as_u64()).unwrap_or(0) * 1000,
                extra: None,
                album: None,
                subtitle: item.pointer("/stat/view").and_then(|x| x.as_u64()).map(play_count),
            })
        })
        .collect())
}

/// 收藏夹（自己的，需要登录）
pub async fn fav_list(media_id: u64, page: u32) -> AppResult<Page<Track>> {
    let url = format!("https://api.bilibili.com/x/v3/fav/resource/list?media_id={media_id}&pn={page}&ps=20&platform=web");
    let v = get_json(&url).await?;
    if code_of(&v) != 0 {
        return Err(AppError::Message(format!("收藏夹读取失败({}): {}", code_of(&v), msg_of(&v))));
    }
    let medias = v.pointer("/data/medias").and_then(|m| m.as_array()).cloned().unwrap_or_default();
    let has_more = v.pointer("/data/has_more").and_then(|h| h.as_bool()).unwrap_or(false);
    let items = medias
        .iter()
        .filter_map(|m| {
            let bvid = m.get("bvid").and_then(|x| x.as_str())?.to_string();
            let upper = m.pointer("/upper/name").and_then(|x| x.as_str()).unwrap_or("").to_string();
            Some(Track {
                source: "bilibili".into(),
                id: bvid,
                title: m.get("title").and_then(|x| x.as_str()).unwrap_or("未命名").to_string(),
                artist: upper,
                cover: cover(m.get("cover").and_then(|x| x.as_str()).unwrap_or("")),
                duration_ms: m.get("duration").and_then(|x| x.as_u64()).unwrap_or(0) * 1000,
                extra: None,
                album: None,
                subtitle: None,
            })
        })
        .collect();
    Ok(Page { items, page, has_more, total: None })
}

/// 我的收藏夹列表
pub async fn fav_folders(mid: &str) -> AppResult<Value> {
    let v = get_json(&format!(
        "https://api.bilibili.com/x/v3/fav/folder/created/list-all?up_mid={mid}&web_location=333.1387"
    ))
    .await?;
    Ok(v)
}

/// 稍后再看 / 最近播放
pub async fn toview() -> AppResult<Vec<Track>> {
    let v = get_json("https://api.bilibili.com/x/v2/history/toview").await?;
    if code_of(&v) != 0 {
        return Ok(Vec::new());
    }
    let list = v.get("data").and_then(|d| d.get("list")).and_then(|l| l.as_array()).cloned().unwrap_or_default();
    Ok(list
        .iter()
        .filter_map(|item| {
            let bvid = item.get("bvid").and_then(|x| x.as_str())?.to_string();
            Some(Track {
                source: "bilibili".into(),
                id: bvid,
                title: item.get("title").and_then(|x| x.as_str()).unwrap_or("未命名").to_string(),
                artist: item.pointer("/owner/name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                cover: cover(item.get("pic").and_then(|x| x.as_str()).unwrap_or("")),
                duration_ms: item.get("duration").and_then(|x| x.as_u64()).unwrap_or(0) * 1000,
                extra: None,
                album: None,
                subtitle: None,
            })
        })
        .collect())
}

#[derive(Deserialize)]
struct DashAudio {
    id: u64,
    #[serde(default)]
    base_url: String,
    #[serde(default, rename = "baseUrl")]
    base_url_camel: String,
    #[serde(default)]
    mime_type: String,
    #[serde(default, rename = "mimeType")]
    mime_type_camel: String,
    #[serde(default)]
    backup_url: Vec<String>,
    #[serde(default, rename = "backupUrl")]
    backup_url_camel: Vec<String>,
}

#[derive(Deserialize, Default)]
struct FlacInfo {
    #[serde(default)]
    audio: Option<DashAudio>,
}

#[derive(Deserialize)]
struct Dash {
    #[serde(default)]
    audio: Vec<DashAudio>,
    #[serde(default)]
    dolby: Option<FlacInfo>,
    #[serde(default)]
    flac: Option<FlacInfo>,
    #[serde(default)]
    duration: u64,
}

#[derive(Deserialize)]
struct PlayUrlData {
    #[serde(default)]
    dash: Option<Dash>,
}

pub struct ResolvedAudio {
    pub url: String,
    pub backups: Vec<String>,
    pub mime: String,
    pub duration_ms: u64,
}

/// 拿音频流地址（dash 里的纯音频轨，不含视频，省流量）
pub async fn resolve_audio(bvid: &str, cid: u64, preferred_qn: Option<u64>) -> AppResult<ResolvedAudio> {
    let v = wbi_get(
        "/x/player/wbi/playurl",
        &[
            ("bvid", bvid.to_string()),
            ("cid", cid.to_string()),
            ("fnval", "16".to_string()),
            ("fnver", "0".to_string()),
            ("fourk", "1".to_string()),
        ],
    )
    .await?;

    let data: PlayUrlData = serde_json::from_value(v["data"].clone())?;
    let dash = data.dash.ok_or_else(|| AppError::Message("该视频没有独立音轨（可能是互动视频或已下架）".into()))?;

    let mut candidates: Vec<(u64, &DashAudio)> = dash.audio.iter().map(|a| (a.id, a)).collect();
    if let Some(f) = dash.flac.as_ref().and_then(|f| f.audio.as_ref()) {
        candidates.push((9999, f));
    }
    if let Some(d) = dash.dolby.as_ref().and_then(|d| d.audio.as_ref()) {
        candidates.push((8888, d));
    }
    candidates.sort_by(|a, b| a.0.cmp(&b.0));
    if let Some(want) = preferred_qn {
        if let Some(pos) = candidates.iter().position(|(id, _)| *id == want) {
            let item = candidates.remove(pos);
            candidates.push(item);
        }
    }
    let (id, audio) = candidates
        .pop()
        .ok_or_else(|| AppError::Message("没有可用音轨".into()))?;
    let _ = id;

    let mut url = audio.base_url.clone();
    if url.is_empty() {
        url = audio.base_url_camel.clone();
    }
    let mut backups = audio.backup_url.clone();
    if backups.is_empty() {
        backups = audio.backup_url_camel.clone();
    }
    let mime = if !audio.mime_type.is_empty() {
        audio.mime_type.clone()
    } else if !audio.mime_type_camel.is_empty() {
        audio.mime_type_camel.clone()
    } else {
        "audio/mp4".to_string()
    };

    if url.is_empty() {
        return Err(AppError::Message("音轨地址为空".into()));
    }
    Ok(ResolvedAudio { url, backups, mime, duration_ms: dash.duration.saturating_mul(1000) })
}

/// 当前登录信息。
/// 关键点：nav 接口在风控/网络异常时会返回非 0，此时不能直接判定"未登录"，
/// 否则用户明明登录了、收藏夹却一直是空的。这里以 cookie 里的 SESSDATA 为准，
/// 昵称/头像只是锦上添花。
pub async fn me() -> UserInfo {
    let sess = session::get();
    let has_cookie = sess.has_login();

    match get_json("https://api.bilibili.com/x/web-interface/nav").await {
        Ok(v) => {
            let logged = v.pointer("/data/isLogin").and_then(|x| x.as_bool()).unwrap_or(false);
            if logged {
                let info = UserInfo {
                    logged_in: true,
                    name: v.pointer("/data/uname").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    avatar: v.pointer("/data/face").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    mid: v
                        .pointer("/data/mid")
                        .and_then(|x| x.as_u64())
                        .map(|m| m.to_string())
                        .unwrap_or_default(),
                };
                let snapshot = info.clone();
                let _ = session::update(|s| {
                    s.bilibili_name = snapshot.name.clone();
                    s.bilibili_avatar = snapshot.avatar.clone();
                });
                return info;
            }
            // 只有 nav 明确回 -101（账号未登录）才说明 cookie 真过期了，可以清。
            // 其它非 0 码（-352 风控、-500 服务异常等）不能当"未登录"处理，
            // 否则用户会被莫名其妙踢下线。
            let code = code_of(&v);
            if has_cookie && code == -101 {
                log::warn!("B 站登录态已失效（-101），清除本地 cookie");
                let _ = session::update(|s| {
                    s.bilibili_cookie.clear();
                    s.bilibili_name.clear();
                    s.bilibili_avatar.clear();
                });
                return UserInfo::anonymous();
            }
            if has_cookie {
                log::warn!("nav 返回 {code}，按本地 cookie 认为已登录");
                return UserInfo {
                    logged_in: true,
                    name: sess.bilibili_name.clone(),
                    avatar: sess.bilibili_avatar.clone(),
                    mid: sess.cookie_value("DedeUserID").unwrap_or_default(),
                };
            }
            UserInfo::anonymous()
        }
        Err(e) => {
            // 网络 / 风控失败：按本地 cookie 判断，至少让收藏夹功能可用
            if has_cookie {
                log::warn!("nav 接口异常（{e}），按本地 cookie 认为已登录");
                UserInfo {
                    logged_in: true,
                    name: sess.bilibili_name.clone(),
                    avatar: sess.bilibili_avatar.clone(),
                    mid: sess.cookie_value("DedeUserID").unwrap_or_default(),
                }
            } else {
                UserInfo::anonymous()
            }
        }
    }
}

/// 扫码登录
pub struct QrCode {
    pub url: String,
    pub key: String,
}

pub async fn qr_generate() -> AppResult<QrCode> {
    let v = get_json("https://passport.bilibili.com/x/passport-login/web/qrcode/generate").await?;
    if code_of(&v) != 0 {
        return Err(AppError::Message(format!("二维码生成失败: {}", msg_of(&v))));
    }
    Ok(QrCode {
        url: v.pointer("/data/url").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        key: v.pointer("/data/qrcode_key").and_then(|x| x.as_str()).unwrap_or("").to_string(),
    })
}

/// 轮询扫码状态：0 成功 / 86101 未扫描 / 86090 已扫描待确认 / 86038 已过期
pub async fn qr_poll(key: &str) -> AppResult<(i64, String)> {
    let url = format!("https://passport.bilibili.com/x/passport-login/web/qrcode/poll?qrcode_key={key}&source=main-fe-header");
    let resp = client()
        .get(&url)
        .header("Referer", "https://www.bilibili.com")
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
    let v: Value = serde_json::from_str(&text)
        .map_err(|_| AppError::Message(format!("扫码返回异常: {}", &text.chars().take(120).collect::<String>())))?;
    let code = v.pointer("/data/code").and_then(|c| c.as_i64()).unwrap_or(-1);
    let message = v.pointer("/data/message").and_then(|m| m.as_str()).unwrap_or("").to_string();

    if code == 0 {
        let incoming = cookies.join("; ");
        let existing = session::get().bilibili_cookie;
        let merged = session::merge_cookie(
            &existing,
            &incoming,
            &["SESSDATA", "bili_jct", "DedeUserID", "DedeUserID__ckMd5", "sid", "buvid3", "buvid4"],
        );
        session::update(|s| s.bilibili_cookie = merged)?;
        if let Ok(cookie_url) = v.pointer("/data/url").and_then(|u| u.as_str()).map(|s| s.to_string()).ok_or(()) {
            // 兜底：从跳转地址里再抓一遍关键 cookie
            let extra: Vec<String> = cookie_url
                .split('?')
                .nth(1)
                .map(|q| {
                    q.split('&')
                        .filter(|kv| ["SESSDATA", "bili_jct", "DedeUserID"].iter().any(|k| kv.starts_with(&format!("{k}="))))
                        .map(|s| s.replace("&", ""))
                        .collect()
                })
                .unwrap_or_default();
            if !extra.is_empty() {
                let cur = session::get().bilibili_cookie;
                let merged = session::merge_cookie(&cur, &extra.join("; "), &["SESSDATA", "bili_jct", "DedeUserID"]);
                session::update(|s| s.bilibili_cookie = merged)?;
            }
        }
    }
    Ok((code, message))
}

pub async fn logout() -> AppResult<()> {
    session::update(|s| {
        s.bilibili_cookie.clear();
        s.bilibili_name.clear();
        s.bilibili_avatar.clear();
    })?;
    Ok(())
}

/// 各音乐分区，作为首页入口
pub fn music_regions() -> Vec<(&'static str, u32)> {
    vec![
        ("音乐区", 3),
        ("国创音乐", 168),
        ("VOCALOID", 30),
        ("演奏", 59),
        ("MV", 193),
        ("音乐现场", 31),
        ("翻唱", 130),
        ("原创音乐", 28),
    ]
}

pub fn region_name(rid: u32) -> String {
    music_regions()
        .into_iter()
        .find(|(_, r)| *r == rid)
        .map(|(n, _)| n.to_string())
        .unwrap_or_else(|| "音乐".to_string())
}

pub type _Unused = HashMap<String, String>;
