//! 本地音频代理的行为测试（离线也能跑一部分）。

use bili_music_lib::proxy;

#[tokio::test]
async fn proxy_registers_token_and_serves_health() {
    let port = proxy::start();
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

/// Windows 的 WebView2 会跟着 Chromium 对「页面 → 127.0.0.1」的请求套
/// Private Network Access 规则，所以本地代理必须答预检，否则音频一播就 error。
#[tokio::test]
async fn proxy_answers_cors_preflight() {
    let port = proxy::start();
    assert!(port > 0);

    let resp = reqwest::Client::new()
        .request(reqwest::Method::OPTIONS, format!("http://127.0.0.1:{port}/stream/any-token"))
        .header("Origin", "http://tauri.localhost")
        .header("Access-Control-Request-Method", "GET")
        .header("Access-Control-Request-Headers", "range")
        .header("Access-Control-Request-Private-Network", "true")
        .send()
        .await
        .expect("预检请求失败");

    assert_eq!(resp.status(), 204, "预检应返回 204");
    let headers = resp.headers();
    assert_eq!(headers.get("access-control-allow-origin").unwrap(), "*");
    assert_eq!(headers.get("access-control-allow-private-network").unwrap(), "true");
    assert!(headers.get("access-control-allow-methods").unwrap().to_str().unwrap().contains("GET"));
}
