use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, Emitter};
use tauri_plugin_shell::ShellExt;
use tokio::sync::mpsc;

#[derive(Clone, Serialize)]
pub struct SyncEvent {
    pub path: String,
    pub timestamp: String,
    pub status: String,
}

pub struct SyncManager {
    app: AppHandle,
    debounce_duration: Duration,
}

impl SyncManager {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            debounce_duration: Duration::from_secs(5),
        }
    }

    pub fn start_watcher(&self, local_path: PathBuf, remote_folder: String) -> Result<(), String> {
        let (tx, mut rx) = mpsc::channel(100);
        let app_handle = self.app.clone();
        let debounce_duration = self.debounce_duration;
        let watch_path = local_path.clone();

        // 1. Start the notify watcher
        std::thread::spawn(move || {
            let mut watcher = RecommendedWatcher::new(
                move |res: notify::Result<notify::Event>| {
                    if let Ok(event) = res {
                        // We only care about actual content changes or new files
                        if event.kind.is_modify() || event.kind.is_create() {
                            for path in event.paths {
                                let _ = tx.blocking_send(path);
                            }
                        }
                    }
                },
                Config::default(),
            )
            .expect("Failed to create watcher");

            watcher
                .watch(&watch_path, RecursiveMode::Recursive)
                .expect("Failed to watch path");

            loop {
                std::thread::sleep(Duration::from_secs(1));
            }
        });

        // 2. Debounce and sync loop
        let last_event_time = Arc::new(Mutex::new(Instant::now()));
        let sync_active = Arc::new(Mutex::new(false));
        let last_changed_path = Arc::new(Mutex::new(PathBuf::new()));

        tokio::spawn(async move {
            while let Some(path) = rx.recv().await {
                {
                    let mut last_time = last_event_time.lock().unwrap();
                    *last_time = Instant::now();
                    let mut last_path = last_changed_path.lock().unwrap();
                    *last_path = path;
                }

                let mut active = sync_active.lock().unwrap();
                if !*active {
                    *active = true;
                    let inner_app = app_handle.clone();
                    let inner_root_path = local_path.clone();
                    let inner_remote_folder = remote_folder.clone();
                    let inner_last_time = Arc::clone(&last_event_time);
                    let inner_sync_active = Arc::clone(&sync_active);
                    let inner_last_path = Arc::clone(&last_changed_path);

                    tokio::spawn(async move {
                        loop {
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            let last_time = inner_last_time.lock().unwrap();
                            if last_time.elapsed() >= debounce_duration {
                                break;
                            }
                        }

                        let target_path = {
                            let p = inner_last_path.lock().unwrap();
                            p.clone()
                        };

                        let _ = trigger_sync(inner_app, inner_root_path, target_path, inner_remote_folder).await;
                        
                        let mut active = inner_sync_active.lock().unwrap();
                        *active = false;
                    });
                }
            }
        });

        Ok(())
    }
}

pub async fn trigger_sync(app: AppHandle, root_path: PathBuf, changed_path: PathBuf, remote_folder: String) -> Result<(), String> {
    let _ = app.emit("sync-start", ());
    
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e: tauri::Error| e.to_string())?
        .join("rclone.conf");

    let config_path = data_dir.to_string_lossy().to_string();
    let root_path_str = root_path.to_string_lossy().to_string();
    
    let folder_name = root_path.file_name().unwrap_or_default().to_string_lossy();
    let remote_dest = format!("gdrive:{}/{}", remote_folder, folder_name);

    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let path_display = changed_path.to_string_lossy().to_string();

    let output = app
        .shell()
        .sidecar("rclone")
        .map_err(|e| e.to_string())?
        .args([
            "--config",
            &config_path,
            "copy",
            &root_path_str,
            &remote_dest,
        ])
        .output()
        .await
        .map_err(|e| e.to_string())?;

    let status = if output.status.success() {
        "Success".to_string()
    } else {
        "Failed".to_string()
    };

    let _ = app.emit("sync-event", SyncEvent {
        path: path_display,
        timestamp,
        status,
    });

    let _ = app.emit("sync-end", ());
    Ok(())
}
