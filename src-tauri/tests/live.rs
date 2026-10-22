//! 联网冒烟测试：需要能访问 B 站。
//! 跑法（并发跑容易被 B 站风控，建议单线程）：
//!   cargo test -p bili-music --test live -- --nocapture --test-threads=1

use bili_music_lib::bilibili;

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
    let page = with_retry(|| bilibili::search("周杰伦", 1)).await;
    assert!(!page.items.is_empty(), "搜索结果为空");
    assert_eq!(page.items[0].source, "bilibili");
    println!("B 站搜索首条: {} / {}", page.items[0].title, page.items[0].artist);
}

#[tokio::test]
async fn bilibili_ranking_works() {
    let page = with_retry(|| bilibili::ranking(3, 1)).await;
    assert!(page.items.len() > 5);
    println!("音乐区排行首条: {}", page.items[0].title);
}

#[tokio::test]
async fn bilibili_related_works() {
    let list = with_retry(|| bilibili::related("BV1GJ411x7h7")).await;
    assert!(!list.is_empty(), "相关推荐为空");
    println!("相关推荐 {} 条，首条: {}", list.len(), list[0].title);
}

#[tokio::test]
async fn bilibili_audio_resolves_to_dash_audio() {
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
    let vid = with_retry(|| bilibili::video_detail("BV1GJ411x7h7")).await;
    assert!(vid.cid > 0);
    assert!(!vid.track.title.is_empty());
    assert!(!vid.pages.is_empty());
    println!("视频: {} cid={} 分P={}", vid.track.title, vid.cid, vid.pages.len());
}
