use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tauri_plugin_shell::ShellExt;
use tokio::sync::mpsc;

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

    pub fn start_watcher(&self, local_path: PathBuf) -> Result<(), String> {
        let (tx, mut rx) = mpsc::channel(100);
        let app_handle = self.app.clone();
        let debounce_duration = self.debounce_duration;
        let watch_path = local_path.clone();

        // 1. Start the notify watcher in a separate thread
        std::thread::spawn(move || {
            let mut watcher = RecommendedWatcher::new(
                move |res: notify::Result<notify::Event>| {
                    if let Ok(_) = res {
                        let _ = tx.blocking_send(());
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

        tokio::spawn(async move {
            while let Some(_) = rx.recv().await {
                {
                    let mut last_time = last_event_time.lock().unwrap();
                    *last_time = Instant::now();
                }

                let mut active = sync_active.lock().unwrap();
                if !*active {
                    *active = true;
                    let inner_app = app_handle.clone();
                    let inner_path = local_path.clone();
                    let inner_last_time = Arc::clone(&last_event_time);
                    let inner_sync_active = Arc::clone(&sync_active);

                    tokio::spawn(async move {
                        loop {
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            let last_time = inner_last_time.lock().unwrap();
                            if last_time.elapsed() >= debounce_duration {
                                break;
                            }
                        }

                        let _ = trigger_sync(inner_app, inner_path).await;
                        
                        let mut active = inner_sync_active.lock().unwrap();
                        *active = false;
                    });
                }
            }
        });

        Ok(())
    }
}

pub async fn trigger_sync(app: AppHandle, local_path: PathBuf) -> Result<(), String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e: tauri::Error| e.to_string())?
        .join("rclone.conf");

    let config_path = data_dir.to_string_lossy().to_string();
    let local_path_str = local_path.to_string_lossy().to_string();
    
    let folder_name = local_path.file_name().unwrap_or_default().to_string_lossy();
    let remote_dest = format!("gdrive:slynk_backup/{}", folder_name);

    println!("Starting rclone sync for {}", local_path_str);

    let output = app
        .shell()
        .sidecar("rclone")
        .map_err(|e| e.to_string())?
        .args([
            "--config",
            &config_path,
            "copy",
            &local_path_str,
            &remote_dest,
        ])
        .output()
        .await
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        println!("Rclone sync completed successfully for {}", folder_name);
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        eprintln!("Rclone sync failed: {}", stderr);
        Err(stderr)
    }
}
