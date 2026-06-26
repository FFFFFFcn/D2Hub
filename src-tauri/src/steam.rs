use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::System;
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;

const STEAMID64_BASE: u64 = 76561197960265728;

/// 从注册表获取 Steam 安装路径
pub fn get_steam_path() -> Result<PathBuf> {
    if let Ok(key) =
        RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\WOW6432Node\Valve\Steam")
    {
        if let Ok(path) = key.get_value::<String, _>("InstallPath") {
            let p = PathBuf::from(path);
            if p.exists() {
                return Ok(p);
            }
        }
    }
    if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Valve\Steam") {
        if let Ok(path) = key.get_value::<String, _>("SteamPath") {
            let p = PathBuf::from(path);
            if p.exists() {
                return Ok(p);
            }
        }
    }
    Err(anyhow!("无法找到 Steam 安装路径"))
}

/// SteamID64 → accountid（userdata 目录名）
pub fn steamid64_to_accountid(steamid64: u64) -> u64 {
    steamid64 - STEAMID64_BASE
}

/// 从 loginusers.vdf 获取所有账号信息: Vec<(SteamID64, AccountName)>
pub fn get_all_users(steam_path: &PathBuf) -> Vec<(u64, String)> {
    let path = steam_path.join("config").join("loginusers.vdf");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let mut users: Vec<(u64, String)> = Vec::new();
    let mut current_id: Option<u64> = None;

    for line in content.lines() {
        let trimmed = line.trim().trim_matches('"');
        if trimmed.len() == 17 && trimmed.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(id) = trimmed.parse::<u64>() {
                current_id = Some(id);
            }
        }
        if line.contains("AccountName") {
            if let Some(id) = current_id.take() {
                let name = line
                    .split('"')
                    .nth(3)
                    .unwrap_or("unknown")
                    .to_string();
                users.push((id, name));
            }
        }
    }
    users
}

/// 获取当前活跃用户的 localconfig.vdf 路径
/// 策略：扫描所有 userdata/*/config/localconfig.vdf，取最新修改的那个（Steam 实时写入活跃账号）
/// 回退：MostRecent=1 的账号
pub fn get_localconfig_path(steam_path: &PathBuf) -> Result<(PathBuf, u64, String)> {
    let userdata_dir = steam_path.join("userdata");

    // 1. 扫描所有 localconfig.vdf，找最新修改的那个
    let mut best: Option<(PathBuf, u64, std::time::SystemTime)> = None;
    if let Ok(entries) = std::fs::read_dir(&userdata_dir) {
        for entry in entries.flatten() {
            let config = entry.path().join("config").join("localconfig.vdf");
            if config.exists() {
                if let Ok(meta) = std::fs::metadata(&config) {
                    if let Ok(mod_time) = meta.modified() {
                        let folder = entry.file_name().to_string_lossy().to_string();
                        if let Ok(accountid) = folder.parse::<u64>() {
                            let steamid64 = accountid + STEAMID64_BASE;
                            match best {
                                Some((_, _, t)) if mod_time > t => {
                                    best = Some((config, steamid64, mod_time));
                                }
                                None => {
                                    best = Some((config, steamid64, mod_time));
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some((path, steamid64, _)) = best {
        let name = get_user_name(steam_path, steamid64);
        return Ok((path, steamid64, name));
    }

    // 2. 回退：使用 loginusers.vdf 中的 MostRecent=1
    let steamid64 = get_most_recent_user(steam_path).or_else(|| {
        get_all_users(steam_path).first().map(|(id, _)| *id)
    });
    if let Some(steamid64) = steamid64 {
        let accountid = steamid64_to_accountid(steamid64);
        let path = userdata_dir
            .join(accountid.to_string())
            .join("config")
            .join("localconfig.vdf");
        if path.exists() {
            let name = get_user_name(steam_path, steamid64);
            return Ok((path, steamid64, name));
        }
    }

    Err(anyhow!("未找到任何用户的 localconfig.vdf"))
}

/// 获取 loginusers.vdf 中 MostRecent="1" 的 SteamID64
fn get_most_recent_user(steam_path: &PathBuf) -> Option<u64> {
    let path = steam_path.join("config").join("loginusers.vdf");
    let content = std::fs::read_to_string(&path).ok()?;
    let mut current_id: Option<u64> = None;
    for line in content.lines() {
        let trimmed = line.trim().trim_matches('"');
        if trimmed.len() == 17 && trimmed.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(id) = trimmed.parse::<u64>() {
                current_id = Some(id);
            }
        }
        if line.contains("MostRecent") && line.contains("\"1\"") {
            return current_id;
        }
    }
    None
}

/// 获取账号名（从 loginusers.vdf）
fn get_user_name(steam_path: &PathBuf, steamid64: u64) -> String {
    let users = get_all_users(steam_path);
    for (id, name) in &users {
        if *id == steamid64 {
            return name.clone();
        }
    }
    steamid64.to_string()
}

fn normalize_path(path: &str) -> String {
    path.replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

/// steam.exe 是否在运行
pub fn is_steam_running() -> bool {
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    sys.processes().values().any(|p| {
        p.exe()
            .and_then(|e| e.to_str())
            .map(|e| normalize_path(e).ends_with("steam.exe"))
            .unwrap_or(false)
    })
}

/// steamwebhelper.exe 进程数
pub fn count_steamwebhelper() -> usize {
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    sys.processes()
        .values()
        .filter(|p| {
            p.exe()
                .and_then(|e| e.to_str())
                .map(|e| normalize_path(e).ends_with("steamwebhelper.exe"))
                .unwrap_or(false)
        })
        .count()
}

/// 优雅关闭 Steam，等待完全退出
pub fn shutdown_steam_graceful(steam_path: &PathBuf, timeout_secs: u64) -> Result<()> {
    let steam_exe = steam_path.join("steam.exe");
    if !is_steam_running() {
        return Ok(());
    }
    Command::new(&steam_exe)
        .arg("-shutdown")
        .spawn()
        .context("无法执行 steam.exe -shutdown")?;

    let start = Instant::now();
    loop {
        if !is_steam_running() && count_steamwebhelper() == 0 {
            thread::sleep(Duration::from_secs(1));
            return Ok(());
        }
        if start.elapsed().as_secs() >= timeout_secs {
            return Err(anyhow!("Steam 未在 {}s 内关闭", timeout_secs));
        }
        thread::sleep(Duration::from_millis(500));
    }
}

/// 获取当前活跃用户的 Steam 头像路径
pub fn get_avatar_path(steam_path: &PathBuf) -> Option<PathBuf> {
    if let Ok((_, steamid64, _)) = get_localconfig_path(steam_path) {
        let avatar = steam_path
            .join("config")
            .join("avatarcache")
            .join(format!("{}.png", steamid64));
        if avatar.exists() {
            return Some(avatar);
        }
    }
    None
}

/// 获取头像 base64 (data URL)
pub fn get_avatar_base64(steam_path: &PathBuf) -> Option<String> {
    let path = get_avatar_path(steam_path)?;
    let bytes = std::fs::read(&path).ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        base64_encode(&bytes)
    ))
}

fn base64_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARS[((n >> 18) & 63) as usize] as char);
        out.push(CHARS[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 { out.push(CHARS[((n >> 6) & 63) as usize] as char); } else { out.push('='); }
        if chunk.len() > 2 { out.push(CHARS[(n & 63) as usize] as char); } else { out.push('='); }
    }
    out
}

/// 启动 Dota 2: steam.exe -applaunch 570 [args]
pub fn launch_dota2(steam_path: &PathBuf, args: &[String]) -> Result<()> {
    let steam_exe = steam_path.join("steam.exe");
    Command::new(&steam_exe)
        .arg("-applaunch")
        .arg("570")
        .args(args)
        .spawn()
        .context("无法启动 steam.exe -applaunch 570")?;
    Ok(())
}
