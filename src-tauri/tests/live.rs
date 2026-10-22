//! 联网冒烟测试：需要能访问 B 站与网易云。
//! 跑法（并发跑容易被 B 站风控，建议单线程）：
//!   cargo test -p bili-music --test live -- --nocapture --test-threads=1

use bili_music_lib::{bilibili, model::Track, netease};

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
async fn bilibili_audio_resolves_to_dash_audio() {
    let vid = bilibili::video_detail("BV1GJ411x7h7").await.expect("视频详情失败");
    let audio = bilibili::resolve_audio(&vid.track.id, vid.cid, None)
        .await
        .expect("解析音轨失败");
    assert!(audio.url.starts_with("http"), "音轨地址异常");
    assert!(!audio.mime.is_empty());
    println!("音轨: {} mime={} 备份={}", &audio.url[..60.min(audio.url.len())], audio.mime, audio.backups.len());
}

#[tokio::test]
async fn netease_search_and_url() {
    let page = netease::search("周杰伦", 1).await.expect("网易云搜索失败");
    assert!(!page.items.is_empty());
    println!("网易云搜索首条: {} / {}", page.items[0].title, page.items[0].artist);

    // 晴天（免费曲目）应当能拿到播放地址
    let track: &Track = &page.items[0];
    match netease::song_url(&track.id, 320_000).await {
        Ok((url, mime, size)) => println!("网易云地址 ok mime={mime} size={size} url={}", &url[..50.min(url.len())]),
        Err(e) => println!("网易云首条无版权或需登录（正常）: {e}"),
    }
}

#[tokio::test]
async fn netease_lyric_and_toplists() {
    let lists = netease::toplists().await.expect("榜单失败");
    assert!(!lists.is_empty());
    println!("榜单数: {} 首个: {}", lists.len(), lists[0].0);

    let (lrc, _tl) = netease::lyric("186016").await.expect("歌词失败");
    assert!(lrc.contains('['), "歌词格式异常");
    println!("歌词行数: {}", lrc.lines().count());
}
