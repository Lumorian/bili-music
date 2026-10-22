use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

use serde::{Deserialize, Serialize};

use crate::error::AppResult;

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();
static STATE: OnceLock<RwLock<Session>> = OnceLock::new();

pub fn set_data_dir(dir: PathBuf) {
    let _ = DATA_DIR.set(dir);
    let _ = STATE.set(RwLock::new(Session::load()));
}

fn state() -> &'static RwLock<Session> {
    STATE.get_or_init(|| RwLock::new(Session::load()))
}

fn file_path() -> Option<PathBuf> {
    DATA_DIR.get().map(|d| d.join("session.json"))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    /// B 站 Cookie 串，形如 "SESSDATA=xx; bili_jct=yy; DedeUserID=zz"
    pub bilibili_cookie: String,
    /// 网易云 Cookie 串
    pub netease_cookie: String,
    pub bilibili_name: String,
    pub bilibili_avatar: String,
    pub netease_name: String,
    pub netease_avatar: String,
    /// 同步服务器地址（一起听）
    pub together_server: String,
    pub nickname: String,
}

impl Session {
    pub fn load() -> Self {
        let Some(p) = file_path() else { return Self::default() };
        std::fs::read_to_string(p)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> AppResult<()> {
        if let Some(p) = file_path() {
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(p, serde_json::to_vec_pretty(self)?)?;
        }
        Ok(())
    }
}

pub fn get() -> Session {
    state().read().map(|s| s.clone()).unwrap_or_default()
}

pub fn update<F: FnOnce(&mut Session)>(f: F) -> AppResult<Session> {
    let mut guard = state().write().map_err(|_| "会话锁被污染")?;
    f(&mut guard);
    let snapshot = guard.clone();
    drop(guard);
    snapshot.save()?;
    Ok(snapshot)
}

/// 从 Set-Cookie 头数组里挑出需要的键值，合并进已有 cookie 串
pub fn merge_cookie(existing: &str, incoming: &str, keep: &[&str]) -> String {
    let mut pairs: Vec<(String, String)> = existing
        .split(';')
        .filter_map(|kv| {
            let mut it = kv.trim().splitn(2, '=');
            let k = it.next()?.trim().to_string();
            let v = it.next().unwrap_or("").trim().to_string();
            if k.is_empty() {
                None
            } else {
                Some((k, v))
            }
        })
        .collect();

    for part in incoming.split(';') {
        let mut it = part.trim().splitn(2, '=');
        let k = it.next().unwrap_or("").trim().to_string();
        let v = it.next().unwrap_or("").trim().to_string();
        if k.is_empty() || v.is_empty() {
            continue;
        }
        if !keep.is_empty() && !keep.iter().any(|want| want.eq_ignore_ascii_case(&k)) {
            continue;
        }
        if let Some(slot) = pairs.iter_mut().find(|(ek, _)| ek == &k) {
            slot.1 = v;
        } else {
            pairs.push((k, v));
        }
    }

    pairs
        .into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}
