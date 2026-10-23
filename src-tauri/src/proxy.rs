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
/// 串行化启动流程：测试里两个用例、或者前端首屏并发调用时，
/// 不能让它们各自 bind 一个端口（否则 register 会拿到 port=0 的实例）。
static START_LOCK: Mutex<()> = Mutex::new(());

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
                    .header("Access-Control-Allow-Origin", "*")
                    .header("Access-Control-Expose-Headers", "content-length, content-range, accept-ranges, etag")
                    .header("Access-Control-Allow-Private-Network", "true");
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

/// CORS 预检。
///
/// 现在的 <audio> 是 no-CORS 请求，本来用不上；但 Windows 的 WebView2 会跟着
/// Chromium 的 Private Network Access 规则，对"页面源 → 127.0.0.1"这类
/// 本地地址的请求做预检。少一个 204 就会表现成「地址拿到了但一播就 error」，
/// 所以这里直接给全，省得以后又踩。
async fn preflight() -> Response {
    (
        StatusCode::NO_CONTENT,
        [
            ("access-control-allow-origin", "*"),
            ("access-control-allow-methods", "GET, HEAD, OPTIONS"),
            ("access-control-allow-headers", "range, accept, origin, content-type"),
            ("access-control-expose-headers", "content-length, content-range, accept-ranges, etag"),
            ("access-control-max-age", "86400"),
            // Private Network Access：允许跨到 loopback
            ("access-control-allow-private-network", "true"),
            ("access-control-allow-credentials", "false"),
        ],
    )
        .into_response()
}

async fn health() -> &'static str {
    "ok"
}

fn build_router(state: ProxyState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/stream/{token}", get(proxy_stream).options(preflight))
        .with_state(state)
}

/// 同步启动本地代理（幂等）。
/// 用 std 的 TcpListener 先绑定拿到端口，再交给 tokio，
/// 这样在 Tauri 的 setup 里调用时不需要 block_on（避免自己等自己）。
pub fn start() -> u16 {
    if let Some(s) = STATE.get() {
        return s.port;
    }
    let _guard = START_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // 抢到锁之后再确认一次：可能有别的线程刚刚启好
    if let Some(s) = STATE.get() {
        return s.port;
    }
    let std_listener = match std::net::TcpListener::bind("127.0.0.1:0") {
        Ok(l) => l,
        Err(e) => {
            log::error!("本地音频代理启动失败: {e}");
            return 0;
        }
    };
    let port = std_listener.local_addr().map(|a| a.port()).unwrap_or(0);
    if std_listener.set_nonblocking(true).is_err() {
        log::error!("无法把音频代理切到非阻塞模式");
        return 0;
    }

    let state = ProxyState { tokens: Default::default(), port };
    let _ = STATE.set(state.clone());
    let app = build_router(state);

    log::info!("本地音频代理已启动: 127.0.0.1:{port}");
    tauri::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::from_std(std_listener) {
            Ok(l) => l,
            Err(e) => {
                log::error!("音频代理接管监听失败: {e}");
                return;
            }
        };
        if let Err(e) = axum::serve(listener, app).await {
            log::error!("音频代理退出: {e}");
        }
    });
    port
}

/// 异步版本的启动入口（测试里用）
pub async fn start_async() -> u16 {
    start()
}

#[allow(dead_code)]
fn _keep(state: ProxyState) -> ProxyState {
    state
}

/// 兜底：尚未启动时启动一次
pub fn ensure_started() -> u16 {
    start()
}
