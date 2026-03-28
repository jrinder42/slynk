# Slynk

Slynk is a cross-platform desktop application built with **Tauri v2** and **Svelte 5** that provides a seamless interface for backing up local folders to cloud storage (specifically Google Drive) using **rclone**.

## Features

- **Real-time Sync:** Uses native file system events to detect changes and trigger syncs.
- **Tray-First UI:** Accessible directly from the system menu bar for a non-intrusive experience.
- **Sidecar Power:** Leverages the robustness of `rclone` for reliable transfers.
- **Persistence:** Remembers your backup configurations and remote settings across restarts.
- **Live Progress Tracking:** View real-time synchronization progress and file transfer status.

## Prerequisites

- [Bun](https://bun.sh/)
- [Rust](https://www.rust-lang.org/)
- [rclone](https://rclone.org/) (Setup handled by internal scripts)

## Development

### 1. Setup Sidecars
Ensure `rclone` binaries are placed correctly:
```bash
bun run setup
```

### 2. Run Development Server
```bash
bun tauri dev
```

### 3. Check Backend
```bash
cargo check
```

## Tech Stack

- **Framework:** Tauri v2
- **Frontend:** Svelte 5, TypeScript, Vite
- **Backend:** Rust (Tokio, Notify)
- **Sidecar:** rclone
