//! BiliMusic：只用来听歌的 B 站客户端 + 一起听
//!
//! 架构：
//!   前端 (React/TS)  ←invoke/event→  本 crate (Tauri)
//!   本 crate 负责：B 站接口(wbi 签名)、本地音频代理、一起听客户端
//!   sync-core：一起听的协议与房间服务器实现（桌面端和 Android 共用）

pub mod bilibili;
pub mod commands;
pub mod error;
pub mod http;
pub mod model;
pub mod proxy;
pub mod session;
pub mod together;
pub mod together_cloud;
pub mod together_cmds;

use std::sync::Arc;

use tauri::Manager;
use tokio::sync::Mutex;

/// 修正 AppImage 里 GStreamer 插件的环境变量。
///
/// linuxdeploy 的 gstreamer 插件把 `GST_PLUGIN_SCANNER_1_0` 写到了一个并不存在的
/// 路径（`usr/lib/gstreamer1.0/gstreamer-1.0/gst-plugin-scanner`，正确的在
/// `usr/lib/gstreamer-1.0/`），于是每个 WebProcess 都会刷一屏
/// "External plugin loader failed" 警告，插件扫描退化成兜底路径。
/// 这里在 WebKit 起来之前把它指回真实位置；只在确认目标缺失时才改，
/// 正常安装（deb/rpm/裸二进制）完全不受影响。
fn fix_gstreamer_env() {
    let Ok(appdir) = std::env::var("APPDIR") else { return };
    if appdir.is_empty() {
        return;
    }
    let bundled = std::path::Path::new(&appdir).join("usr/lib/gstreamer-1.0");

    // 1) plugin scanner
    let scanner = std::env::var("GST_PLUGIN_SCANNER_1_0").unwrap_or_default();
    if !scanner.is_empty() && !std::path::Path::new(&scanner).exists() {
        let fixed = bundled.join("gst-plugin-scanner");
        if fixed.exists() {
            std::env::set_var("GST_PLUGIN_SCANNER_1_0", &fixed);
            std::env::set_var("GST_PLUGIN_SCANNER", &fixed);
            log::info!("已修正 GST_PLUGIN_SCANNER: {}", fixed.display());
        }
    }

    // 2) 插件目录：确保至少指向包里那套
    let sys = std::env::var("GST_PLUGIN_SYSTEM_PATH_1_0").unwrap_or_default();
    if sys.is_empty() && bundled.exists() {
        std::env::set_var("GST_PLUGIN_SYSTEM_PATH_1_0", &bundled);
        std::env::set_var("GST_PLUGIN_PATH_1_0", &bundled);
        log::info!("已设置 GStreamer 插件目录: {}", bundled.display());
    }
}

/// 当前时间戳（毫秒）
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 日志插件。
///
/// 默认配置是「stdout + 系统日志目录文件」。但日志目录不可写时
/// （只读的 $HOME、被沙箱限制的容器、某些 Linux 发行版的 /run/user 只读等），
/// **插件初始化会直接 panic，整个应用起不来**——这不该是"日志写不了"的后果。
///
/// 所以这里先自己试建一次目录：能建就带上文件目标，建不了就只往 stdout 打。
fn log_plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    use tauri_plugin_log::{Target, TargetKind};

    let mut targets = vec![Target::new(TargetKind::Stdout)];

    // 优先用应用数据目录下的 logs/，和会话文件放一起，方便用户打包发给我们
    let dir = std::env::var("XDG_DATA_HOME")
        .ok()
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var("HOME").ok().map(|h| std::path::PathBuf::from(h).join(".local/share")))
        .map(|base| base.join("dev.lumorian.bilimusic").join("logs"));

    if let Some(dir) = dir {
        if std::fs::create_dir_all(&dir).is_ok() {
            // 再试写一个探针文件：有些环境目录能建、文件却写不进去
            let probe = dir.join(".write_probe");
            if std::fs::write(&probe, b"ok").is_ok() {
                let _ = std::fs::remove_file(&probe);
                targets.push(Target::new(TargetKind::Folder { path: dir, file_name: None }));
            } else {
                log::warn!("日志目录不可写，只输出到 stdout");
            }
        }
    }

    tauri_plugin_log::Builder::new().targets(targets).build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    fix_gstreamer_env();
    tauri::Builder::default()
        .plugin(log_plugin())
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
            commands::get_settings,
            commands::save_settings,
            commands::app_info,
            together_cmds::together_cloud_host,
            together_cmds::together_cloud_join,
            together_cmds::together_cloud_probe,
            together_cmds::together_am_host,
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
