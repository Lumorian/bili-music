//! 本地音频代理。
//! 作用有三个：
//! 1. 给 B 站音频流补上 Referer/UA，否则 CDN 会 403；
//! 2. 把 http 上游转成 http://127.0.0.1:port/stream/<token>，WebView 里可直接 <audio src>；
//! 3. 透传 Range，保证拖动进度条能秒定位。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use crate::http::stream_client;

#[derive(Clone)]
struct StreamEntry {
    urls: Vec<String>,
    referer: String,
    mime: String,
    created: Instant,
}

#[derive(Clone, Default)]
pub struct ProxyState {
    tokens: std::sync::Arc<Mutex<HashMap<String, StreamEntry>>>,
    pub port: u16,
}

static STATE: OnceLock<ProxyState> = OnceLock::new();

pub fn state() -> &'static ProxyState {
    STATE.get_or_init(ProxyState::default)
}

impl ProxyState {
    /// 注册一个上游地址，返回 (token, 本地代理地址)
    pub fn register(&self, url: String, backups: Vec<String>, referer: String, mime: String) -> (String, String) {
        let token = uuid::Uuid::new_v4().simple().to_string();
        let mut urls = vec![url];
        urls.extend(backups);
        // 顺手清理过期 token，避免内存一直涨
        if let Ok(mut map) = self.tokens.lock() {
            if map.len() > 64 {
                map.retain(|_, v| v.created.elapsed() < Duration::from_secs(3 * 3600));
            }
            map.insert(
                token.clone(),
                StreamEntry { urls, referer, mime, created: Instant::now() },
            );
        }
        (token.clone(), format!("http://127.0.0.1:{}/stream/{}", self.port, token))
    }
}

fn mime_of(entry: &StreamEntry) -> String {
    if entry.mime.is_empty() {
        "audio/mp4".to_string()
    } else {
        entry.mime.clone()
    }
}

async fn proxy_stream(
    State(state): State<ProxyState>,
    Path(token): Path<String>,
    headers: HeaderMap,
) -> Response {
    let entry = {
        let map = match state.tokens.lock() {
            Ok(m) => m,
            Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "lock poisoned").into_response(),
        };
        map.get(&token).cloned()
    };
    let Some(entry) = entry else {
        return (StatusCode::NOT_FOUND, "stream token expired").into_response();
    };

    let range = headers.get(axum::http::header::RANGE).and_then(|v| v.to_str().ok()).map(|s| s.to_string());
    let referer = if entry.referer.is_empty() { "https://www.bilibili.com".to_string() } else { entry.referer.clone() };

    let mut last_err = String::from("no upstream");
    for url in &entry.urls {
        let mut req = stream_client()
            .get(url)
            .header("Referer", &referer)
            .header("Origin", referer.trim_end_matches('/'))
            .header("Accept", "*/*");
        if let Some(r) = &range {
            req = req.header("Range", r);
        }
        match req.send().await {
            Ok(resp) => {
                let status = resp.status();
                if !status.is_success() {
                    last_err = format!("upstream {status}");
                    continue;
                }
                let mut builder = Response::builder()
                    .status(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK))
                    .header("Content-Type", mime_of(&entry))
                    .header("Accept-Ranges", "bytes")
                    .header("Cache-Control", "no-store")
                    .header("Access-Control-Allow-Origin", "*");
                for h in ["content-length", "content-range", "etag", "last-modified"] {
                    if let Some(v) = resp.headers().get(h).and_then(|v| v.to_str().ok()) {
                        if let Ok(hv) = HeaderValue::from_str(v) {
                            builder = builder.header(h, hv);
                        }
                    }
                }
                let stream = resp.bytes_stream();
                return builder
                    .body(Body::from_stream(stream))
                    .unwrap_or_else(|_| (StatusCode::INTERNAL_SERVER_ERROR, "build response failed").into_response());
            }
            Err(e) => last_err = e.to_string(),
        }
    }
    (StatusCode::BAD_GATEWAY, format!("upstream unreachable: {last_err}")).into_response()
}

async fn health() -> &'static str {
    "ok"
}

/// 启动本地代理（幂等）。桌面端与安卓端都用 127.0.0.1。
pub async fn start() -> u16 {
    if let Some(s) = STATE.get() {
        return s.port;
    }
    let listener = match tokio::net::TcpListener::bind("127.0.0.1:0").await {
        Ok(l) => l,
        Err(e) => {
            log::error!("本地音频代理启动失败: {e}");
            return 0;
        }
    };
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    let state = ProxyState { tokens: Default::default(), port };
    let _ = STATE.set(state.clone());

    let app = Router::new()
        .route("/health", get(health))
        .route("/stream/{token}", get(proxy_stream))
        .with_state(state);

    log::info!("本地音频代理已启动: 127.0.0.1:{port}");
    tauri::async_runtime::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            log::error!("音频代理退出: {e}");
        }
    });
    port
}

pub fn ensure_started() -> u16 {
    if let Some(s) = STATE.get() {
        return s.port;
    }
    // 用一次性阻塞等待来拿到端口（命令调用都在 async 环境里，这里只是兜底）
    tauri::async_runtime::block_on(start())
}
