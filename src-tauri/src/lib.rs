mod sync;
mod db;

use std::sync::Mutex;
use tauri_plugin_shell::ShellExt;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    Manager, State,
};

// Application state to hold the SyncManager instance
struct AppState {
    sync_manager: Mutex<Option<sync::SyncManager>>,
    db: Mutex<db::Db>,
}

#[tauri::command]
async fn test_rclone(app: tauri::AppHandle) -> Result<String, String> {
    // Check rclone version to verify the sidecar is bundled and working
    let output = app
        .shell()
        .sidecar("rclone")
        .map_err(|e| e.to_string())?
        .arg("version")
        .output()
        .await
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

#[tauri::command]
async fn is_authenticated(app: tauri::AppHandle) -> Result<bool, String> {
    // Check if the rclone.conf file exists in the app's data directory
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("rclone.conf");
    Ok(data_dir.exists())
}

#[tauri::command]
async fn rclone_login(app: tauri::AppHandle) -> Result<String, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("rclone.conf");

    // Ensure parent directory exists for the configuration file
    if let Some(parent) = data_dir.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let config_path = data_dir.to_string_lossy().to_string();

    // Run rclone config create to initiate Google Drive authentication
    let output = app
        .shell()
        .sidecar("rclone")
        .map_err(|e| e.to_string())?
        .args([
            "--config",
            &config_path,
            "config",
            "create",
            "gdrive",
            "drive",
            "scope=drive.file",
        ])
        .output()
        .await
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok("Successfully authenticated with Google Drive".to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(format!("Rclone error: {}", stderr))
    }
}

#[tauri::command]
async fn start_backup(app: tauri::AppHandle, state: State<'_, AppState>, paths: Vec<String>, remote_folder: String, batch_size: u32) -> Result<String, String> {
    // Ensure the user is authenticated before starting any background work
    let authenticated = is_authenticated(app.clone()).await?;
    if !authenticated {
        return Err("Not authenticated with Google Drive. Please log in first.".to_string());
    }

    let mut sm_guard = state.sync_manager.lock().unwrap();
    // Lazy-initialize the SyncManager
    if sm_guard.is_none() {
        *sm_guard = Some(sync::SyncManager::new(app.clone()));
    }
    
    // Start watching each selected path
    if let Some(ref sm) = *sm_guard {
        for path in paths {
            sm.start_watcher(std::path::PathBuf::from(path), remote_folder.clone(), batch_size).map_err(|e| e.to_string())?;
        }
    }
    
    Ok("Backup monitoring started for all selected items".to_string())
}

#[tauri::command]
async fn stop_all_monitoring(state: State<'_, AppState>) -> Result<(), String> {
    // Clearing the state drops the SyncManager and its associated watchers
    let mut sm_guard = state.sync_manager.lock().unwrap();
    *sm_guard = None; 
    Ok(())
}

#[tauri::command]
async fn rclone_logout(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    // 1. Stop all active background monitoring
    stop_all_monitoring(state.clone()).await?;

    // 2. Clear the persistent file index
    {
        let db_guard = state.db.lock().unwrap();
        db_guard.clear_index().map_err(|e| e.to_string())?;
    }

    // 3. Remove the authentication tokens
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("rclone.conf");

    if data_dir.exists() {
        std::fs::remove_file(data_dir).map_err(|e| e.to_string())?;
    }
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
            // Initialize the SQLite database in the app data directory
            let data_dir = app.path().app_data_dir().expect("Failed to get app data dir");
            std::fs::create_dir_all(&data_dir).expect("Failed to create app data dir");
            let db_path = data_dir.join("slynk_index.db");
            let db = db::Db::new(&db_path).expect("Failed to initialize database");

            // Manage application state
            app.manage(AppState {
                sync_manager: Mutex::new(None),
                db: Mutex::new(db),
            });

            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Tray configuration
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit_item])?;

            let _tray = TrayIconBuilder::with_id("main-tray")
                .tooltip("slynk")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .on_menu_event(|app, event| {
                    if event.id.as_ref() == "quit" {
                        app.exit(0);
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let is_visible = window.is_visible().unwrap_or(false);
                            let is_focused = window.is_focused().unwrap_or(false);

                            if is_visible && is_focused {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.unminimize();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            test_rclone,
            is_authenticated,
            rclone_login,
            rclone_logout,
            start_backup,
            stop_all_monitoring,
            remove_from_index,
            save_config,
            load_config
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
