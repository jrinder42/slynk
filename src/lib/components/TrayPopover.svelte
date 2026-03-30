<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";

  let status = $state("Up to date");
  let isSyncing = $state(false);
  let isPaused = $state(false);
  let progress = $state(0);
  let currentFile = $state("");
  let recentFiles = $state<string[]>([]);

  onMount(async () => {
    const currentStatus = await invoke("load_config", { key: "monitoringActive" });
    if (currentStatus === false) {
      isPaused = true;
      status = "Paused";
    }

    const unlistenStart = listen("sync-start", () => {
      isSyncing = true;
      status = "Syncing...";
    });

    const unlistenProgress = listen("sync-progress", (event: any) => {
      const data = event.payload;
      progress = data.percentage;
      if (data.current_files && data.current_files.length > 0) {
        currentFile = data.current_files[0];
        if (!recentFiles.includes(currentFile)) {
          recentFiles = [currentFile, ...recentFiles].slice(0, 5);
        }
      }
    });

    const unlistenEnd = listen("sync-end", () => {
      setTimeout(() => {
        isSyncing = false;
        if (!isPaused) status = "Up to date";
        progress = 0;
        currentFile = "";
      }, 1000);
    });

    return () => {
      unlistenStart.then((fn) => fn());
      unlistenProgress.then((fn) => fn());
      unlistenEnd.then((fn) => fn());
    };
  });

  async function openGoogleDrive() {
    await invoke("open_google_drive");
  }

  async function toggleSync() {
    if (isPaused) {
      await invoke("resume_backup");
      isPaused = false;
      status = "Up to date";
      await invoke("save_config", { key: "monitoringActive", value: true });
    } else {
      await invoke("stop_all_monitoring");
      isPaused = true;
      status = "Paused";
      await invoke("save_config", { key: "monitoringActive", value: false });
    }
  }
</script>

<div class="popover minimalist">
  <header>
    <div class="title-group">
      <div class="logo">S</div>
      <h1>Slynk</h1>
    </div>
    <span class="status-badge" class:active={isSyncing} class:paused={isPaused}>
      {#if isSyncing}
        <span class="spinner"></span>
      {/if}
      {status}
    </span>
  </header>
  
  <main>
    {#if isPaused}
      <div class="idle-state">
        <div class="pause-icon">II</div>
        <p>Syncing is currently paused.</p>
      </div>
    {:else if isSyncing}
      <div class="sync-card">
        <div class="progress-info">
          <span class="file-name">{currentFile || 'Preparing files...'}</span>
          <span class="percent">{Math.round(progress)}%</span>
        </div>
        <div class="progress-track">
          <div class="progress-fill" style="width: {progress}%"></div>
        </div>
      </div>
    {:else}
      <div class="idle-state">
        <div class="check-icon">✓</div>
        <p>All files are secured in the cloud.</p>
      </div>
    {/if}

    {#if recentFiles.length > 0}
      <div class="recent-section">
        <h3>Recently Updated</h3>
        <ul>
          {#each recentFiles as file}
            <li>
              <span class="file-icon">📄</span>
              <span class="recent-name">{file}</span>
            </li>
          {/each}
        </ul>
      </div>
    {/if}
  </main>

  <footer>
    <div class="footer-buttons">
      <button class="action-btn" onclick={toggleSync}>
        {isPaused ? 'Resume Sync' : 'Pause Sync'}
      </button>
      <button class="primary-btn" onclick={openGoogleDrive}>
        View Drive
      </button>
    </div>
  </footer>
</div>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    font-family: 'Inter', -apple-system, sans-serif;
  }

  .popover {
    width: 300px;
    background: white;
    padding: 16px;
    border-radius: 12px;
    box-shadow: 0 10px 25px rgba(0,0,0,0.15);
    display: flex;
    flex-direction: column;
    gap: 16px;
    border: 1px solid #eee;
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .title-group {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .logo {
    width: 24px;
    height: 24px;
    background: #2563eb;
    color: white;
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 800;
    font-size: 14px;
  }

  h1 {
    font-size: 16px;
    margin: 0;
    font-weight: 700;
    color: #111827;
  }

  .status-badge {
    font-size: 11px;
    font-weight: 700;
    padding: 4px 8px;
    border-radius: 20px;
    background: #f3f4f6;
    color: #6b7280;
    display: flex;
    align-items: center;
    gap: 6px;
    text-transform: uppercase;
  }

  .status-badge.active {
    background: #dbeafe;
    color: #2563eb;
  }

  .status-badge.paused {
    background: #fef2f2;
    color: #ef4444;
  }

  .spinner {
    width: 14px;
    height: 14px;
    border: 3px solid #2563eb;
    border-top-color: #dbeafe;
    border-radius: 50%;
    animation: spin 0.8s cubic-bezier(0.5, 0, 0.5, 1) infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .sync-card {
    background: #f9fafb;
    padding: 12px;
    border-radius: 8px;
    border: 1px solid #f3f4f6;
  }

  .progress-info {
    display: flex;
    justify-content: space-between;
    margin-bottom: 8px;
    font-size: 12px;
  }

  .file-name {
    color: #374151;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 200px;
  }

  .percent {
    color: #2563eb;
    font-weight: 700;
  }

  .progress-track {
    height: 6px;
    background: #e5e7eb;
    border-radius: 3px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: #2563eb;
    transition: width 0.3s ease;
  }

  .idle-state {
    text-align: center;
    padding: 20px 0;
  }

  .check-icon {
    width: 32px;
    height: 32px;
    background: #ecfdf5;
    color: #059669;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    margin: 0 auto 8px;
    font-weight: 700;
  }

  .pause-icon {
    width: 32px;
    height: 32px;
    background: #fef2f2;
    color: #ef4444;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    margin: 0 auto 8px;
    font-weight: 900;
    font-size: 14px;
  }

  .idle-state p {
    font-size: 13px;
    color: #6b7280;
    margin: 0;
  }

  .recent-section h3 {
    font-size: 11px;
    text-transform: uppercase;
    color: #9ca3af;
    margin: 0 0 8px 0;
    letter-spacing: 0.05em;
  }

  .recent-section ul {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .recent-section li {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: #4b5563;
  }

  .recent-name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  footer {
    border-top: 1px solid #f3f4f6;
    padding-top: 12px;
  }

  .footer-buttons {
    display: flex;
    gap: 8px;
  }

  .action-btn {
    flex: 1;
    background: white;
    color: #374151;
    border: 1px solid #d1d5db;
    padding: 8px;
    border-radius: 6px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }

  .action-btn:hover {
    background: #f9fafb;
    border-color: #9ca3af;
  }

  .primary-btn {
    flex: 1;
    background: #111827;
    color: white;
    border: none;
    padding: 8px;
    border-radius: 6px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.2s;
  }

  .primary-btn:hover {
    background: #374151;
  }
</style>
