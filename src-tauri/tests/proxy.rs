//! 本地音频代理的行为测试（离线也能跑一部分）。

use bili_music_lib::proxy;

#[tokio::test]
async fn proxy_registers_token_and_serves_health() {
    let port = proxy::start().await;
    assert!(port > 0, "代理端口应大于 0");

    let url = format!("http://127.0.0.1:{port}/health");
    let body = reqwest::get(&url).await.expect("健康检查失败").text().await.unwrap();
    assert_eq!(body, "ok");

    let (_, stream_url) = proxy::state().register(
        "http://example.invalid/audio.m4a".to_string(),
        Vec::new(),
        "https://www.bilibili.com".to_string(),
        "audio/mp4".to_string(),
    );
    assert!(stream_url.starts_with(&format!("http://127.0.0.1:{port}/stream/")));

    // 不存在的 token 必须 404，而不是把错误一路带进播放器
    let bad = reqwest::get(format!("http://127.0.0.1:{port}/stream/nope")).await.unwrap();
    assert_eq!(bad.status(), 404);
}
