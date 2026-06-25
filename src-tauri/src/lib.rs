mod config;
mod steam;
mod vdf;

use config::AppConfig;
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};

struct AppState {
    config: Mutex<AppConfig>,
}

// ─── Tauri 命令（供前端设置窗口调用）─────────────────────

#[derive(serde::Serialize)]
pub struct StatusInfo {
    pub steam_path: String,
    pub active_user_id: String,
    pub active_user_name: String,
    pub stored_launch_options: String,
    pub last_server: String,
    pub cleaned: bool,
    pub extra_args: Vec<String>,
    pub steam_running: bool,
}

#[tauri::command]
fn get_status(state: tauri::State<AppState>) -> Result<StatusInfo, String> {
    let steam_path = steam::get_steam_path().map_err(|e| e.to_string())?;
    let (localconfig, steamid64, user_name) =
        steam::get_localconfig_path(&steam_path).map_err(|e| e.to_string())?;
    let stored = vdf::read_launch_options(&localconfig).unwrap_or_default();
    let cfg = state.config.lock().unwrap();
    Ok(StatusInfo {
        steam_path: steam_path.display().to_string(),
        active_user_id: steamid64.to_string(),
        active_user_name: user_name,
        stored_launch_options: stored,
        last_server: cfg.last_server.clone(),
        cleaned: cfg.cleaned,
        extra_args: cfg.extra_args.clone(),
        steam_running: steam::is_steam_running(),
    })
}

#[tauri::command]
async fn launch_server(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    server: String,
) -> Result<String, String> {
    do_launch(&app, &state, &server).await
}

#[tauri::command]
async fn cleanup_options(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let steam_path = steam::get_steam_path().map_err(|e| e.to_string())?;
    let (localconfig, _steamid64, _user_name) =
        steam::get_localconfig_path(&steam_path).map_err(|e| e.to_string())?;
    let stored = vdf::read_launch_options(&localconfig).unwrap_or_default();

    if stored.is_empty() {
        return Ok("启动选项已为空，无需清理".to_string());
    }

    let _ = app.emit("status", "正在关闭 Steam…");
    if steam::is_steam_running() {
        steam::shutdown_steam_graceful(&steam_path, 25).map_err(|e| e.to_string())?;
    }

    vdf::clear_launch_options(&localconfig).map_err(|e| e.to_string())?;

    {
        let mut cfg = state.config.lock().unwrap();
        cfg.cleaned = true;
        config::save(&cfg).map_err(|e| e.to_string())?;
    }

    Ok("启动选项已清理".to_string())
}

#[tauri::command]
fn set_extra_args(state: tauri::State<AppState>, args: Vec<String>) -> Result<(), String> {
    let mut cfg = state.config.lock().unwrap();
    cfg.extra_args = args;
    config::save(&cfg).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("settings") {
        w.show().map_err(|e| e.to_string())?;
        w.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ─── 核心启动逻辑 ─────────────────────────────────────

async fn do_launch(
    app: &AppHandle,
    state: &AppState,
    server: &str,
) -> Result<String, String> {
    let steam_path = steam::get_steam_path().map_err(|e| e.to_string())?;
    let (localconfig, _steamid64, _user_name) =
        steam::get_localconfig_path(&steam_path).map_err(|e| e.to_string())?;
    let stored = vdf::read_launch_options(&localconfig).unwrap_or_default();

    // 一次性清理：若 Steam 启动选项中仍有残留值
    if !stored.is_empty() {
        let _ = app.emit("status", "首次清理：正在关闭 Steam…");
        if steam::is_steam_running() {
            steam::shutdown_steam_graceful(&steam_path, 25).map_err(|e| e.to_string())?;
        }
        vdf::clear_launch_options(&localconfig).map_err(|e| e.to_string())?;
        {
            let mut cfg = state.config.lock().unwrap();
            cfg.cleaned = true;
            config::save(&cfg).map_err(|e| e.to_string())?;
        }
    }

    // 构建启动参数
    let mut args: Vec<String> = Vec::new();
    if server == "cn" {
        args.push("-perfectworld".to_string());
    }
    let extra: Vec<String> = {
        let cfg = state.config.lock().unwrap();
        cfg.extra_args.clone()
    };
    args.extend(extra);

    steam::launch_dota2(&steam_path, &args).map_err(|e| e.to_string())?;

    // 持久化选择
    {
        let mut cfg = state.config.lock().unwrap();
        cfg.last_server = server.to_string();
        config::save(&cfg).map_err(|e| e.to_string())?;
    }

    let name = if server == "cn" { "国服" } else { "全球服" };
    let msg = format!("已启动 Dota 2 - {}", name);
    let _ = app.emit("status", &msg);
    Ok(msg)
}

// ─── 入口 ────────────────────────────────────────────

pub fn run() {
    let initial_config = config::load().unwrap_or_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            config: Mutex::new(initial_config),
        })
        .setup(|app| {
            // 构建托盘菜单
            let launch_cn =
                MenuItem::with_id(app, "launch_cn", "🇨🇳  启动国服", true, None::<&str>)?;
            let launch_global =
                MenuItem::with_id(app, "launch_global", "🌍  启动全球服", true, None::<&str>)?;
            let sep1 = PredefinedMenuItem::separator(app)?;
            let settings = MenuItem::with_id(app, "settings", "⚙️  设置…", true, None::<&str>)?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let quit = MenuItem::with_id(app, "quit", "🚪  退出", true, None::<&str>)?;

            let menu = Menu::with_items(app, &[&launch_cn, &launch_global, &sep1, &settings, &sep2, &quit])?;

            // 托盘图标（使用应用默认图标）
            let mut tray = TrayIconBuilder::new().menu(&menu).tooltip("Dota2 切换器");
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.on_menu_event(|app, event| match event.id.as_ref() {
                "launch_cn" => {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let state: tauri::State<AppState> = app.state();
                        let _ = do_launch(&app, &state, "cn").await;
                    });
                }
                "launch_global" => {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let state: tauri::State<AppState> = app.state();
                        let _ = do_launch(&app, &state, "global").await;
                    });
                }
                "settings" => {
                    if let Some(w) = app.get_webview_window("settings") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            })
            .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            launch_server,
            cleanup_options,
            set_extra_args,
            open_settings_window,
        ])
        .run(tauri::generate_context!())
        .expect("启动失败");
}
