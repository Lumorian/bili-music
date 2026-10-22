//! 一起听房间服务器。
//!
//! 用法：
//!   bili-music-sync-server              # 监听 0.0.0.0:8787
//!   BIND=0.0.0.0:9000 bili-music-sync-server
//!
//! 客户端在 App 的「一起听」页面填入这个地址即可。

use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let bind = std::env::var("BIND").unwrap_or_else(|_| "0.0.0.0:8787".to_string());
    let addr: SocketAddr = bind.parse().unwrap_or_else(|_| {
        eprintln!("BIND 格式不对: {bind}，应形如 0.0.0.0:8787");
        std::process::exit(1);
    });

    println!("BiliMusic 一起听服务器");
    println!("  监听地址 : http://{addr}");
    println!("  健康检查 : http://{addr}/health");
    println!("  建房接口 : POST http://{addr}/api/rooms");
    println!("  实时通道 : ws://{addr}/ws/<房间号>");

    if let Err(e) = bili_music_sync::server::serve(addr).await {
        eprintln!("服务器退出: {e}");
        std::process::exit(1);
    }
}
