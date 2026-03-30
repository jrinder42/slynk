mod sync;
mod db;

use std::sync::{Arc, Mutex};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_shell::ShellExt;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    Manager, State,
};

struct AppState {
    sync_manager: Mutex<Option<sync::SyncManager>>,
    db: Mutex<db::Db>,
}

#[tauri::command]
async fn open_google_drive(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store(".settings.dat").map_err(|e| e.to_string())?;
    
    let remote_folder = store.get("remoteFolder")
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "slynk_backup".to_string());

    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("rclone.conf");
    let config_path = data_dir.to_string_lossy().to_string();

    // 1. Get user email dynamically using JSON output
    let user_info_output = app.shell().sidecar("rclone")
        .map_err(|e| e.to_string())?
        .args(["--config", &config_path, "config", "userinfo", "gdrive:", "--json"])
        .output()
        .await;

    let mut email_hint = String::new();
    if let Ok(out) = user_info_output {
        if out.status.success() {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                if let Some(email) = json.get("email").and_then(|v| v.as_str()) {
                    email_hint = email.to_string();
                }
            }
        }
    }

    // 2. Attempt to get the ID of the folder
    let output = app.shell().sidecar("rclone")
        .map_err(|e| e.to_string())?
        .args([
            "--config", &config_path,
            "lsf",
            &format!("gdrive:{}", remote_folder),
            "--format", "i",
            "--directory-only",
            "--max-depth", "1"
        ])
        .output()
        .await;

    let mut url = format!("https://drive.google.com/drive/search?q={}", remote_folder);

    if let Ok(out) = output {
        if out.status.success() {
            let id = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !id.is_empty() {
                url = format!("https://drive.google.com/drive/folders/{}", id);
            }
        }
    }

    // 3. Append the authuser hint if we found an email dynamically
    if !email_hint.is_empty() {
        let separator = if url.contains('?') { "&" } else { "?" };
        url = format!("{}{}authuser={}", url, separator, email_hint);
    }
        
    let _ = app.opener().open_url(url, None::<&str>);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]

async fn test_rclone(app: tauri::AppHandle) -> Result<String, String> {
    let output = app.shell().sidecar("rclone").map_err(|e| e.to_string())?.arg("version").output().await.map_err(|e| e.to_string())?;
    if output.status.success() { Ok(String::from_utf8_lossy(&output.stdout).to_string()) } else { Err(String::from_utf8_lossy(&output.stderr).to_string()) }
}

#[tauri::command]
async fn is_authenticated(app: tauri::AppHandle) -> Result<bool, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("rclone.conf");
    Ok(data_dir.exists())
}

#[tauri::command]
async fn rclone_login(app: tauri::AppHandle) -> Result<String, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("rclone.conf");
    if let Some(parent) = data_dir.parent() { std::fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
    let config_path = data_dir.to_string_lossy().to_string();
    let output = app.shell().sidecar("rclone").map_err(|e| e.to_string())?.args(["--config", &config_path, "config", "userinfo", "gdrive:"]).output().await.map_err(|e| e.to_string())?;
    if output.status.success() { Ok("Authenticated".to_string()) } else { Err(format!("Error: {}", String::from_utf8_lossy(&output.stderr))) }
}

#[tauri::command]
async fn start_backup(app: tauri::AppHandle, state: State<'_, AppState>, paths: Vec<String>, remote_folder: String, batch_size: u32) -> Result<String, String> {
    let authenticated = is_authenticated(app.clone()).await?;
    if !authenticated { return Err("Not authenticated".to_string()); }
    let mut sm_guard = state.sync_manager.lock().unwrap();
    if sm_guard.is_none() { *sm_guard = Some(sync::SyncManager::new(app.clone())); }
    if let Some(ref sm) = *sm_guard {
        for path in paths { sm.start_watcher(std::path::PathBuf::from(path), remote_folder.clone(), batch_size).map_err(|e| e.to_string())?; }
    }
    Ok("Started".to_string())
}

#[tauri::command]
async fn stop_all_monitoring(state: State<'_, AppState>) -> Result<(), String> {
    let mut sm_guard = state.sync_manager.lock().unwrap();
    *sm_guard = None; 
    Ok(())
}

#[tauri::command]
async fn rclone_logout(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    stop_all_monitoring(state.clone()).await?;
    { let db_guard = state.db.lock().unwrap(); db_guard.clear_index().map_err(|e| e.to_string())?; }
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("rclone.conf");
    if data_dir.exists() { std::fs::remove_file(data_dir).map_err(|e| e.to_string())?; }
    Ok(())
}

#[tauri::command]
async fn remove_from_index(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let db_guard = state.db.lock().unwrap();
    db_guard.remove_path(&path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn save_config(app: tauri::AppHandle, key: String, value: serde_json::Value) -> Result<(), String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store(".settings.dat").map_err(|e| e.to_string())?;
    store.set(key, value);
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn load_config(app: tauri::AppHandle, key: String) -> Result<serde_json::Value, String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store(".settings.dat").map_err(|e| e.to_string())?;
    Ok(store.get(key).unwrap_or(serde_json::Value::Null))
}

#[tauri::command]
async fn resume_backup(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    use tauri_plugin_store::StoreExt;
    let store = app.store(".settings.dat").map_err(|e| e.to_string())?;
    let paths = store.get("backupItems").and_then(|v| v.as_array().cloned()).unwrap_or_default().into_iter().filter(|i| i["enabled"].as_bool().unwrap_or(false)).filter_map(|i| i["path"].as_str().map(|s| s.to_string())).collect::<Vec<String>>();
    if paths.is_empty() { return Err("No folders enabled.".to_string()); }
    let remote_folder = store.get("remoteFolder").and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_else(|| "slynk_backup".to_string());
    let batch_size = store.get("batchSize").and_then(|v| v.as_u64()).unwrap_or(1000) as u32;
    start_backup(app, state, paths, remote_folder, batch_size).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir().expect("Failed to get app data dir");
            std::fs::create_dir_all(&data_dir).expect("Failed to create app data dir");
            let db_path = data_dir.join("slynk_index.db");
            let db = db::Db::new(&db_path).expect("Failed to initialize database");
            app.manage(AppState { sync_manager: Mutex::new(None), db: Mutex::new(db) });
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit_item])?;
            let _tray = TrayIconBuilder::with_id("main-tray")
                .tooltip("slynk")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| { if event.id.as_ref() == "quit" { app.exit(0); } })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { position, .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("popover") {
                            let is_visible = window.is_visible().unwrap_or(false);
                            if is_visible { let _ = window.hide(); } else { let _ = window.set_position(position); let _ = window.show(); let _ = window.set_focus(); }
                        }
                    }
                })
                .build(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            test_rclone, is_authenticated, rclone_login, rclone_logout, start_backup, stop_all_monitoring,
            remove_from_index, save_config, load_config, open_google_drive, resume_backup
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
