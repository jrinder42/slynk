use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::ShellExt;
use tokio::sync::mpsc;

pub struct SyncManager {
    app: AppHandle,
    debounce_duration: Duration,
    // Store watchers so they don't get dropped
    _watchers: Arc<Mutex<Vec<RecommendedWatcher>>>,
}

impl SyncManager {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            debounce_duration: Duration::from_secs(5),
            _watchers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn start_watcher(&self, local_path: PathBuf, remote_folder: String, batch_size: u32) -> Result<(), String> {
        let app_handle = self.app.clone();
        let debounce_duration = self.debounce_duration;
        let watch_path = local_path.clone();
        let remote_folder_inner = remote_folder.clone();

        // 1. "Missed Files" Recovery: Background Sweep using SQLite
        let sweep_app = app_handle.clone();
        let sweep_path = local_path.clone();
        let sweep_remote = remote_folder.clone();
        
        tokio::spawn(async move {
            let changed_files = {
                let state = sweep_app.state::<crate::AppState>();
                let db_guard = state.db.lock().unwrap();
                db_guard.get_changed_files(&sweep_path)
            };

            match changed_files {
                Ok(files) => {
                    if !files.is_empty() {
                        println!("Found {} changed items. Starting background sweep...", files.len());
                        if let Err(e) = trigger_sync(sweep_app, sweep_path, PathBuf::from("Sweep"), sweep_remote, batch_size).await {
                            eprintln!("Sweep failed: {}", e);
                        }
                    } else {
                        println!("No changes detected since last run.");
                    }
                }
                Err(e) => eprintln!("Failed to scan disk for changes: {}", e),
            }
        });

        // 2. Start the notify watcher
        let (tx, mut rx) = mpsc::channel(100);
        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
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

        self._watchers.lock().unwrap().push(watcher);

        // 3. Debounce and sync loop
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
                    let inner_remote_folder = remote_folder_inner.clone();
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

                        let _ = trigger_sync(inner_app, inner_root_path, target_path, inner_remote_folder, batch_size).await;
                        
                        let mut active = inner_sync_active.lock().unwrap();
                        *active = false;
                    });
                }
            }
        });

        Ok(())
    }
}

#[derive(serde::Serialize, Clone)]
struct SyncProgress {
    percentage: f64,
    speed: String,
    eta: String,
    current_files: Vec<String>,
}

pub async fn trigger_sync(app: AppHandle, root_path: PathBuf, changed_path: PathBuf, remote_folder: String, batch_size: u32) -> Result<(), String> {
    let _ = app.emit("sync-start", ());
    
    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_tooltip(Some("Slynk - Syncing..."));
        #[cfg(target_os = "macos")]
        let _ = tray.set_title(Some(" (Syncing...)")); 
    }

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

    // Use --use-json-log and --stats for real-time progress parsing
    let (mut rx, mut _child) = app
        .shell()
        .sidecar("rclone")
        .map_err(|e| e.to_string())?
        .args([
            "--config",
            &config_path,
            "copy",
            &root_path_str,
            &remote_dest,
            "--use-json-log",
            "--stats",
            "1s",
            "--stats-one-line",
        ])
        .spawn()
        .map_err(|e| e.to_string())?;

    let app_inner = app.clone();
    let root_path_inner = root_path.clone();

    tokio::spawn(async move {
        use tauri_plugin_shell::process::CommandEvent;

        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line_bytes) => {
                    let line = String::from_utf8_lossy(&line_bytes);
                    println!("[Rclone]: {}", line.trim());
                }
                CommandEvent::Stderr(line_bytes) => {
                    let line = String::from_utf8_lossy(&line_bytes);
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                        if let Some(stats) = json.get("stats") {
                            // ... stats parsing ...
                            let percentage = stats.get("percentage").and_then(|v| v.as_f64()).unwrap_or(0.0);
                            let speed = stats.get("speed").and_then(|v| v.as_f64()).map(|v| format!("{:.2} MB/s", v / 1024.0 / 1024.0)).unwrap_or_default();
                            let eta = stats.get("eta").and_then(|v| v.as_i64()).map(|v| format!("{}s", v)).unwrap_or_default();
                            
                            let mut current_files = Vec::new();
                            if let Some(transferring) = stats.get("transferring").and_then(|v| v.as_array()) {
                                for f in transferring {
                                    if let Some(name) = f.get("name").and_then(|v| v.as_str()) {
                                        current_files.push(name.to_string());
                                    }
                                }
                            }

                            let _ = app_inner.emit("sync-progress", SyncProgress {
                                percentage,
                                speed,
                                eta,
                                current_files,
                            });
                        }
                    } else {
                        println!("[Rclone Log]: {}", line.trim());
                    }
                }
                CommandEvent::Terminated(payload) => {
                    if payload.code == Some(0) {
                        println!("[{}] SUCCESS: Sync complete for {}.", timestamp, path_display);
                        // Update the persistent index
                        let update_app = app_inner.clone();
                        let update_path = root_path_inner.clone();
                        tokio::spawn(async move {
                            println!("Updating file index database...");
                            update_recursive_index(&update_app, &update_path, batch_size).await;
                            println!("Index update complete.");
                        });
                    } else {
                        eprintln!("[{}] FAILED: Sync of {} failed with status {:?}", timestamp, path_display, payload.code);
                    }
                }
                _ => {}
            }
        }
        let _ = app_inner.emit("sync-end", ());

        // Change tray icon back to idle
        if let Some(tray) = app_inner.tray_by_id("main-tray") {
            let _ = tray.set_tooltip(Some("Slynk - Up to date"));
            #[cfg(target_os = "macos")]
            let _ = tray.set_title(Some(""));
        }
    });

    Ok(())
}

async fn update_recursive_index(app: &AppHandle, root_path: &std::path::Path, batch_size: u32) {
    let state = app.state::<crate::AppState>();
    let mut db_guard = state.db.lock().unwrap();
    let _ = db_guard.update_directory_index_batch(root_path, batch_size);
}
