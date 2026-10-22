//! BiliMusic：只用来听歌的 B 站客户端 + 网易云一起听
//!
//! 架构：
//!   前端 (React/TS)  ←invoke/event→  本 crate (Tauri)
//!   本 crate 负责：B 站接口(wbi 签名)、网易云接口、本地音频代理、一起听客户端
//!   sync-core：一起听的协议与房间服务器实现（桌面端和 Android 共用）

pub mod bilibili;
pub mod commands;
pub mod error;
pub mod http;
pub mod model;
pub mod netease;
pub mod proxy;
pub mod session;
pub mod together;
pub mod together_cmds;

use std::sync::Arc;

use tauri::Manager;
use tokio::sync::Mutex;

/// 当前时间戳（毫秒）
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .setup(|app| {
            // 数据目录：会话、设置都放这里
            let dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir().join("bili-music"));
            let _ = std::fs::create_dir_all(&dir);
            session::set_data_dir(dir);

            // 本地音频代理（WebView 里 <audio> 直接吃这个地址）
            let port = proxy::start();
            if port == 0 {
                log::error!("音频代理启动失败，播放功能可能不可用");
            }

            app.manage(Arc::new(together::Together::default()));
            log::info!("BiliMusic 启动完成，代理端口 {port}");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::resolve_stream,
            commands::bili_search,
            commands::bili_ranking,
            commands::bili_related,
            commands::bili_detail,
            commands::bili_regions,
            commands::bili_toview,
            commands::bili_fav_folders,
            commands::bili_fav_list,
            commands::bili_me,
            commands::bili_qr,
            commands::bili_qr_poll,
            commands::bili_logout,
            commands::ne_search,
            commands::ne_song,
            commands::ne_lyric,
            commands::ne_toplists,
            commands::ne_playlist,
            commands::ne_my_playlists,
            commands::ne_me,
            commands::ne_qr,
            commands::ne_qr_poll,
            commands::ne_logout,
            commands::get_settings,
            commands::save_settings,
            commands::app_info,
            together_cmds::together_create,
            together_cmds::together_join,
            together_cmds::together_snapshot,
            together_cmds::together_leave,
            together_cmds::together_chat,
            together_cmds::together_set_queue,
            together_cmds::together_add_track,
            together_cmds::together_remove_track,
            together_cmds::together_play_index,
            together_cmds::together_set_playing,
            together_cmds::together_seek,
            together_cmds::together_next,
            together_cmds::together_prev,
            together_cmds::together_progress,
            together_cmds::together_transfer_host,
            together_cmds::together_kick,
            together_cmds::together_default_server,
            together_cmds::together_ping,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

type _UnusedMutex = Mutex<()>;
