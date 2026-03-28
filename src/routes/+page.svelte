<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  interface BackupItem {
    path: string;
    isDirectory: boolean;
    enabled: boolean;
  }

  let authStatus = $state("Not connected");
  let rcloneVersion = $state("Checking...");
  let backupItems = $state<BackupItem[]>([]);
  let backupStatus = $state("Idle");
  let remoteFolder = $state("slynk_backup");
  let batchSize = $state(1000);
  let pendingBatchSize = $state(1000);
  let showSavedBatch = $state(false);
  let isSyncing = $state(false);
  let isInitialLoading = $state(true);
  let syncProgress = $state(0);
  let syncSpeed = $state("");
  let syncEta = $state("");
  let currentFiles = $state<string[]>([]);

  onMount(async () => {
    // 1. Check if we're authenticated
    try {
      const authenticated = await invoke("is_authenticated");
      if (authenticated) {
        authStatus = "Connected!";
      } else {
        authStatus = "Not connected";
      }
    } catch (error) {
      console.error("Failed to check auth status:", error);
    }

    // 2. Load initial settings
    try {
      const savedRemote = await invoke("load_config", { key: "remoteFolder" });
      if (savedRemote) remoteFolder = savedRemote as string;

      const savedBatch = await invoke("load_config", { key: "batchSize" });
      if (savedBatch) {
        batchSize = savedBatch as number;
        pendingBatchSize = batchSize;
      }

      const savedItems = await invoke("load_config", { key: "backupItems" });
      if (savedItems) backupItems = savedItems as BackupItem[];

      // 3. Resume monitoring if we're authenticated AND have enabled items
      if (authStatus === "Connected!") {
        const enabledPaths = backupItems.filter(i => i.enabled).map(i => i.path);
        if (enabledPaths.length > 0) {
          await startMonitoring();
        }
      }
    } catch (error) {
      console.error("Failed to load settings:", error);
    } finally {
      isInitialLoading = false;
    }

    // 4. Listen for sync status events
    const unlistenStart = listen("sync-start", () => {
      isSyncing = true;
      syncProgress = 0;
      currentFiles = [];
      backupStatus = "Syncing...";
    });
    const unlistenProgress = listen("sync-progress", (event: any) => {
      const data = event.payload;
      syncProgress = data.percentage;
      syncSpeed = data.speed;
      syncEta = data.eta;
      currentFiles = data.current_files;
    });
    const unlistenEnd = listen("sync-end", () => {
      isSyncing = false;
      syncProgress = 0;
      currentFiles = [];
      backupStatus = "Monitoring...";
    });

    return () => {
      unlistenStart.then(fn => fn());
      unlistenProgress.then(fn => fn());
      unlistenEnd.then(fn => fn());
    };
  });

  // 4. Auto-save settings whenever they change
  $effect(() => {
    if (!isInitialLoading) {
      invoke("save_config", { key: "backupItems", value: backupItems });
    }
  });

  $effect(() => {
    if (!isInitialLoading) {
      invoke("save_config", { key: "remoteFolder", value: remoteFolder });
    }
  });

  async function saveBatchSize() {
    batchSize = pendingBatchSize;
    await invoke("save_config", { key: "batchSize", value: batchSize });
    showSavedBatch = true;
    setTimeout(() => showSavedBatch = false, 1500);
  }

  function handleBatchKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      saveBatchSize();
    }
  }

  async function login() {
    try {
      authStatus = "Starting rclone authentication...";
      await invoke("rclone_login");
      authStatus = "Connected!";
      // Auto-start monitoring after login
      await startMonitoring();
    } catch (error) {
      authStatus = `Error: ${error}`;
    }
  }

  async function logout() {
    try {
      await invoke("rclone_logout");
      authStatus = "Not connected";
    } catch (error) {
      console.error("Logout error:", error);
    }
  }

  async function checkRclone() {
    try {
      rcloneVersion = await invoke("test_rclone");
    } catch (error) {
      rcloneVersion = `Error: ${error}`;
    }
  }

  async function addItems(isDirectory: boolean) {
    const selected = await open({
      directory: isDirectory,
      multiple: true,
    });
    
    if (selected) {
      const paths = Array.isArray(selected) ? selected : [selected];
      
      paths.forEach(newPath => {
        // 1. Check if the new path is a subfolder of something already in the list
        const isRedundant = backupItems.some(existing => 
          newPath.startsWith(existing.path + '/') || newPath.startsWith(existing.path + '\\') || newPath === existing.path
        );

        if (isRedundant) return;

        // 2. If the new path is a PARENT of existing items, remove the redundant children
        backupItems = backupItems.filter(existing => 
          !(existing.path.startsWith(newPath + '/') || existing.path.startsWith(newPath + '\\'))
        );

        // 3. Add the new path
        backupItems.push({ path: newPath, isDirectory, enabled: true });
      });
    }
  }

  async function startMonitoring() {
    const enabledPaths = backupItems.filter(i => i.enabled).map(i => i.path);
    if (enabledPaths.length === 0) return;
    
    try {
      backupStatus = "Monitoring...";
      await invoke("start_backup", { 
        paths: enabledPaths,
        remoteFolder,
        batchSize
      });
    } catch (error) {
      backupStatus = `Error: ${error}`;
    }
  }

  function removeItem(path: string) {
    backupItems = backupItems.filter(i => i.path !== path);
    // Also remove from the SQLite index to keep it clean
    invoke("remove_from_index", { path });
  }

  $effect(() => {
    checkRclone();
  });
</script>

<main class="container">
  <h1>slynk</h1>

  <div class="content">
    <div class="section auth-section">
      <div class="header-with-status">
        <p class="version-label">{rcloneVersion}</p>
      </div>
      <p>Status: <strong>{authStatus}</strong></p>
      
      {#if isSyncing}
        <div class="sync-progress-container">
          <div class="progress-bar-bg">
            <div class="progress-bar-fill" style="width: {syncProgress}%"></div>
          </div>
          <div class="sync-stats">
            <span>{Math.round(syncProgress)}% Complete</span>
          </div>
        </div>
      {/if}

      <div class="button-row">
        <button onclick={login} disabled={authStatus === 'Connected!'}>
          {authStatus === 'Connected!' ? 'Connected' : 'Connect to Google Drive'}
        </button>
        {#if authStatus === 'Connected!'}
          <button class="danger-btn" onclick={logout}>Disconnect</button>
        {/if}
      </div>
    </div>

    <div class="section">
      <h3>1. Remote Destination</h3>
      <div class="row">
        <span class="prefix">gdrive:</span>
        <input type="text" bind:value={remoteFolder} placeholder="Enter remote folder name..." />
      </div>
      <p class="help-text">The folder on Google Drive where your files will be mirrored.</p>
    </div>

    <div class="section">
      <h3>2. Performance Settings</h3>
      <div class="row settings-row">
        <span class="label-box">Index Batch Size:</span>
        <input 
          type="number" 
          bind:value={pendingBatchSize} 
          min="100" 
          max="10000" 
          step="100" 
          onkeydown={handleBatchKeydown}
        />
        <button class="submit-btn" onclick={saveBatchSize}>Submit</button>
        {#if showSavedBatch}
          <span class="saved-indicator">✓ Saved</span>
        {/if}
      </div>
      <p class="help-text">Controls RAM usage during large scans. Smaller = less RAM, but slightly slower.</p>
    </div>

    <div class="section">
      <div class="header-with-status">
        <h3>3. Local Items</h3>
        {#if backupItems.length > 0}
          <button class="danger-btn" style="padding: 4px 10px; font-size: 0.75rem;" onclick={() => backupItems = []}>Clear All</button>
        {/if}
      </div>
      <div class="button-row">
        <button onclick={() => addItems(true)}>+ Add Folder</button>
        <button onclick={() => addItems(false)}>+ Add File</button>
      </div>

      <div class="item-list">
        {#if backupItems.length === 0}
          <p class="empty-msg">No items selected yet.</p>
        {:else}
          {#each backupItems as item}
            <div class="item-row">
              <input type="checkbox" bind:checked={item.enabled} />
              <span class="icon">{item.isDirectory ? '📁' : '📄'}</span>
              <div class="item-info" title={item.path}>
                <span class="item-path">{item.path}</span>
              </div>
              <button class="remove-btn" onclick={() => removeItem(item.path)}>×</button>
            </div>
          {/each}
        {/if}
      </div>

      <div class="footer">
        <div class="footer-left">
          {#if isSyncing && currentFiles.length > 0}
            <div class="syncing-files">
              <span class="syncing-title">Syncing Now:</span>
              {#each currentFiles.slice(0, 3) as file}
                <div class="syncing-file-item">
                  <span class="mini-icon">📄</span> {file}
                </div>
              {/each}
              {#if currentFiles.length > 3}
                <div class="syncing-more">... and {currentFiles.length - 3} more</div>
              {/if}
            </div>
          {:else if authStatus === 'Connected!'}
            <p class="status-msg">✅ Monitoring your files</p>
          {/if}
        </div>
      </div>
    </div>
  </div>
</main>

<style>
.container {
  min-height: 100vh;
  margin: 0;
  padding: 24px;
  display: flex;
  flex-direction: column;
  background-color: #f6f6f6;
  box-sizing: border-box;
  overflow-y: auto;
}

@media (prefers-color-scheme: dark) {
  .container {
    color: #f6f6f6;
    background-color: #242424;
  }
}

h1 {
  font-size: 1.8rem;
  margin: 0 0 20px 0;
  text-align: center;
  font-weight: 700;
}

.content {
  max-width: 600px;
  margin: 0 auto;
  width: 100%;
}

.section {
  background: white;
  padding: 16px;
  border-radius: 12px;
  margin-bottom: 16px;
  border: 1px solid #e0e0e0;
  box-shadow: 0 2px 4px rgba(0,0,0,0.05);
}

@media (prefers-color-scheme: dark) {
  .section {
    background: #2f2f2f;
    border-color: #3f3f3f;
  }
}

.header-with-status {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}

.version-label {
  font-size: 0.7rem;
  opacity: 0.5;
  margin: 0;
}

.prefix, .label-box {
  font-weight: 700;
  opacity: 0.8;
  padding: 0 12px;
  background: #f0f0f0;
  border-radius: 6px 0 0 6px;
  border: 1px solid #ccc;
  border-right: none;
  font-size: 0.85rem;
  display: flex;
  align-items: center;
  color: #333;
  height: 36px;
  box-sizing: border-box;
}

@media (prefers-color-scheme: dark) {
  .prefix, .label-box {
    background: #3d3d3d;
    border-color: #555;
    color: #fff;
    opacity: 1;
  }
}

.row input[type="number"] {
  width: 100px;
  font-weight: 700;
  padding: 0 12px;
  border-radius: 0;
  border: 1px solid #ccc;
  height: 36px;
  box-sizing: border-box;
  appearance: textfield;
  -moz-appearance: textfield;
}

.submit-btn {
  border-radius: 0 6px 6px 0;
  border: 1px solid #ccc;
  border-left: none;
  background: #eee;
  color: #333;
  font-weight: 600;
  height: 36px;
  padding: 0 16px;
  display: flex;
  align-items: center;
  box-sizing: border-box;
}

.submit-btn:hover {
  background: #e0e0e0;
}

@media (prefers-color-scheme: dark) {
  .row input[type="number"] {
    background-color: #1e1e1e;
    color: #fff;
    border-color: #555;
  }
  
  .submit-btn {
    background: #444;
    border-color: #555;
    color: #fff;
  }
  
  .submit-btn:hover {
    background: #555;
  }
}

.row input[type="number"]::-webkit-outer-spin-button,
.row input[type="number"]::-webkit-inner-spin-button {
  -webkit-appearance: none;
  margin: 0;
}

.row {
  display: flex;
  gap: 0px;
  align-items: stretch;
}

.settings-row {
  align-items: center;
}

.saved-indicator {
  color: #28a745;
  font-size: 0.75rem;
  font-weight: 700;
  margin-left: 12px;
  animation: fade-in-out 1.5s ease-in-out forwards;
}

@keyframes fade-in-out {
  0% { opacity: 0; transform: translateX(-5px); }
  20% { opacity: 1; transform: translateX(0); }
  80% { opacity: 1; }
  100% { opacity: 0; }
}

.row input {
  border-top-left-radius: 0;
  border-bottom-left-radius: 0;
  flex: 1;
  background-color: #fff;
  color: #000;
  border: 1px solid #ccc;
  padding: 8px 12px;
  font-size: 0.9rem;
}

@media (prefers-color-scheme: dark) {
  .row input {
    background-color: #1e1e1e;
    color: #fff;
    border-color: #555;
  }
}

.danger-btn {
  color: #ff4d4f;
  border-color: #ffa39e;
}

.danger-btn:hover:not(:disabled) {
  background-color: #fff1f0;
}

@media (prefers-color-scheme: dark) {
  .danger-btn {
    border-color: #822a2a;
    background-color: transparent;
  }
  .danger-btn:hover:not(:disabled) {
    background-color: #4c1d1d;
  }
}

.button-row {
  display: flex;
  gap: 8px;
  margin-bottom: 16px;
}

.item-list {
  background: #fdfdfd;
  border: 1px solid #eee;
  border-radius: 8px;
  max-height: 300px;
  overflow-y: auto;
  margin-bottom: 16px;
}

@media (prefers-color-scheme: dark) {
  .item-list {
    background: #2a2a2a;
    border-color: #3a3a3a;
  }
}

.item-row {
  display: flex;
  align-items: center;
  padding: 10px 12px;
  gap: 12px;
  flex-wrap: nowrap;
  overflow: hidden;
  border-bottom: 1px solid rgba(0,0,0,0.05);
}

@media (prefers-color-scheme: dark) {
  .item-row {
    border-bottom-color: rgba(255,255,255,0.05);
  }
}

.item-info {
  flex: 1;
  display: flex;
  min-width: 0;
  overflow: hidden;
}

.item-path {
  font-weight: 600;
  font-size: 0.85rem;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  color: #333;
}

@media (prefers-color-scheme: dark) {
  .item-path {
    color: #fff;
  }
}

.empty-msg {
  text-align: center;
  padding: 32px;
  opacity: 0.5;
  font-style: italic;
  color: #666;
}

@media (prefers-color-scheme: dark) {
  .empty-msg {
    color: #aaa;
  }
}

input[type="checkbox"] {
  width: 18px;
  height: 18px;
  cursor: pointer;
}

button {
  cursor: pointer;
  border-radius: 6px;
  border: 1px solid #ccc;
  padding: 6px 12px;
  font-size: 0.85em;
  transition: all 0.2s;
  background-color: #fff;
}

button:hover:not(:disabled) {
  background-color: #f0f0f0;
}

@media (prefers-color-scheme: dark) {
  button {
    background-color: #3d3d3d;
    color: #fff;
    border-color: #4d4d4d;
  }
  button:hover:not(:disabled) {
    background-color: #4d4d4d;
  }
}

input[type="text"] {
  padding: 6px 10px;
  border-radius: 6px;
  border: 1px solid #ccc;
  font-family: inherit;
  font-size: 0.9em;
  outline: none;
}

@media (prefers-color-scheme: dark) {
  input[type="text"] {
    background-color: #3d3d3d;
    color: #fff;
    border-color: #4d4d4d;
  }
}
</style>
