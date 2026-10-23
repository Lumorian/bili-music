//! 联网冒烟测试：需要能访问 B 站。
//! 跑法（并发跑容易被 B 站风控，建议单线程）：
//!   cargo test -p bili-music --test live -- --nocapture --test-threads=1

use bili_music_lib::bilibili;

/// 这些用例都会打 B 站接口，并发跑必被风控（-352），
/// 所以进程内串行执行：`cargo test --workspace` 不会加 --test-threads=1，
/// 光靠文档里让人手动加参数迟早会被忘掉。
static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// B 站会按 IP 限流，并发跑测试偶尔返回 -352。这里退避重试几次。
async fn with_retry<T, F, Fut>(mut f: F) -> T
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, bili_music_lib::error::AppError>>,
{
    let mut last = None;
    for attempt in 0..4 {
        match f().await {
            Ok(v) => return v,
            Err(e) => {
                last = Some(e);
                tokio::time::sleep(std::time::Duration::from_millis(400 * (attempt + 1))).await;
            }
        }
    }
    panic!("重试后仍失败: {}", last.expect("至少失败一次"));
}

#[tokio::test]
async fn bilibili_search_works() {
    let _guard = SERIAL.lock().await;
    let page = with_retry(|| bilibili::search("周杰伦", 1)).await;
    assert!(!page.items.is_empty(), "搜索结果为空");
    assert_eq!(page.items[0].source, "bilibili");
    println!("B 站搜索首条: {} / {}", page.items[0].title, page.items[0].artist);
}

#[tokio::test]
async fn bilibili_ranking_works() {
    let _guard = SERIAL.lock().await;
    let page = with_retry(|| bilibili::ranking(3, 1)).await;
    assert!(page.items.len() > 5);
    println!("音乐区排行首条: {}", page.items[0].title);
}

#[tokio::test]
async fn bilibili_related_works() {
    let _guard = SERIAL.lock().await;
    let list = with_retry(|| bilibili::related("BV1GJ411x7h7")).await;
    assert!(!list.is_empty(), "相关推荐为空");
    println!("相关推荐 {} 条，首条: {}", list.len(), list[0].title);
}

#[tokio::test]
async fn bilibili_audio_resolves_to_dash_audio() {
    let _guard = SERIAL.lock().await;
    let vid = with_retry(|| bilibili::video_detail("BV1GJ411x7h7")).await;
    let audio = bilibili::resolve_audio(&vid.track.id, vid.cid, None)
        .await
        .expect("解析音轨失败");
    assert!(audio.url.starts_with("http"), "音轨地址异常");
    assert!(!audio.mime.is_empty());
    // 音轨必须是纯音频，而不是把视频流塞进来
    assert!(audio.mime.starts_with("audio/"), "拿到的是视频流: {}", audio.mime);
    println!(
        "音轨: {} mime={} 备份={}",
        &audio.url[..60.min(audio.url.len())],
        audio.mime,
        audio.backups.len()
    );
}

#[tokio::test]
async fn bilibili_video_detail_has_pages() {
    let _guard = SERIAL.lock().await;
    let vid = with_retry(|| bilibili::video_detail("BV1GJ411x7h7")).await;
    assert!(vid.cid > 0);
    assert!(!vid.track.title.is_empty());
    assert!(!vid.pages.is_empty());
    println!("视频: {} cid={} 分P={}", vid.track.title, vid.cid, vid.pages.len());
}

/// 端到端：搜索一条结果 → 解析音轨 → 走本地代理拿真实字节。
/// 这是「按了播放到底有没有声音」的完整链路（Windows/Linux/Android 共用同一份代码）。
#[tokio::test]
async fn proxy_serves_real_audio_bytes() {
    let _guard = SERIAL.lock().await;
    use bili_music_lib::{bilibili, proxy};

    let port = proxy::start();
    assert!(port > 0, "代理没起来");

    let page = with_retry(|| bilibili::search("纯音乐", 1)).await;
    let track = page.items.into_iter().next().expect("搜索无结果");

    let info = bili_music_lib::commands::resolve_stream_inner(&track).await.expect("解析音轨失败");
    assert!(info.stream_url.starts_with(&format!("http://127.0.0.1:{port}/stream/")));
    assert!(info.mime.starts_with("audio/"), "拿到的是视频流: {}", info.mime);

    // 浏览器 <audio> 上来就是 Range 请求，代理必须回 206 + Content-Range
    let resp = reqwest::Client::new()
        .get(&info.stream_url)
        .header("Range", "bytes=0-1023")
        .send()
        .await
        .expect("代理请求失败");
    assert_eq!(resp.status(), 206, "Range 请求应返回 206");
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap().to_string();
    assert!(ct.starts_with("audio/"), "代理返回的 Content-Type 不对: {ct}");
    assert!(resp.headers().get("content-range").is_some(), "缺 Content-Range，拖动进度条会失效");
    let body = resp.bytes().await.expect("读响应体失败");
    assert_eq!(body.len(), 1024, "Range 透传长度不对");
    // AAC/FLAC 都可能有 ID3 或 ftyp 头，这里只要求不是 HTML 错误页
    assert!(!body.starts_with(b"<html"), "上游返回的是错误页");
    println!("代理链路 OK: {ct} 首包 {} 字节", body.len());
}
