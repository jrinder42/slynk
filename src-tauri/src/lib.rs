mod sync;

use tauri_plugin_shell::ShellExt;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    Manager,
};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn test_rclone(app: tauri::AppHandle) -> Result<String, String> {
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
async fn rclone_login(app: tauri::AppHandle) -> Result<String, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("rclone.conf");

    // Ensure parent directory exists
    if let Some(parent) = data_dir.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let config_path = data_dir.to_string_lossy().to_string();

    // Run rclone config create
    // We use --config to specify our own config file location
    // We use gdrive as the remote name
    // drive is the type
    // scope is drive.file
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
async fn start_backup(app: tauri::AppHandle, path: String) -> Result<String, String> {
    let sync_manager = sync::SyncManager::new(app);
    sync_manager.start_watcher(std::path::PathBuf::from(path)).map_err(|e| e.to_string())?;
    Ok("Backup monitoring started".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_sql::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, test_rclone, rclone_login, start_backup])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Create a simple menu with a "Quit" item
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit_item])?;

            // Build the tray icon
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
                            let is_visible: bool = window.is_visible().unwrap_or(false);
                            if is_visible {
                                let _ = window.hide();
                            } else {
                                let _ = window.unminimize();
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
