mod config;
mod steam;
mod vdf;

use config::AppConfig;
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WindowEvent,
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
fn set_extra_args(state: tauri::State<AppState>, args: Vec<String>) -> Result<(), String> {
    let mut cfg = state.config.lock().unwrap();
    cfg.extra_args = args;
    config::save(&cfg).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_avatar_path() -> Result<String, String> {
    let steam_path = steam::get_steam_path().map_err(|e| e.to_string())?;
    steam::get_avatar_base64(&steam_path)
        .ok_or_else(|| "未找到头像".to_string())
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
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 第二个实例启动时，聚焦已有窗口
            if let Some(w) = app.get_webview_window("settings") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .manage(AppState {
            config: Mutex::new(initial_config),
        })
        .setup(|app| {
            // 拦截设置窗口关闭：隐藏而非退出（必须先于托盘创建注册）
            if let Some(w) = app.get_webview_window("settings") {
                let handle = app.handle().clone();
                w.on_window_event(move |e| {
                    if let WindowEvent::CloseRequested { api, .. } = e {
                        api.prevent_close();
                        let _ = handle.get_webview_window("settings").map(|w| w.hide());
                    }
                });
            }

            // 构建托盘菜单
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit])?;

            // 托盘图标（使用应用默认图标）
            let mut tray = TrayIconBuilder::new().menu(&menu).tooltip("D2Copilot");
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.on_menu_event(|app, event| match event.id.as_ref() {
                "quit" => app.exit(0),
                _ => {}
            })
            .on_tray_icon_event(|tray, event| {
                if let tauri::tray::TrayIconEvent::Click {
                    button: tauri::tray::MouseButton::Left, ..
                } = event
                {
                    let handle = tray.app_handle().clone();
                    let _ = handle.get_webview_window("settings").map(|w| {
                        let _ = w.show();
                        let _ = w.set_focus();
                    });
                }
            })
            .build(app)?;

            // 启动时弹出主界面
            if let Some(w) = app.get_webview_window("settings") {
                let _ = w.show();
                let _ = w.set_focus();
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            launch_server,
            set_extra_args,
            get_avatar_path,
            open_settings_window,
        ])
        .run(tauri::generate_context!())
        .expect("启动失败");
}
