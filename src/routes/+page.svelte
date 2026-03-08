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
  let isSyncing = $state(false);

  onMount(() => {
    // Listen for sync status events
    const unlistenStart = listen("sync-start", () => {
      isSyncing = true;
      backupStatus = "Syncing...";
    });
    const unlistenEnd = listen("sync-end", () => {
      isSyncing = false;
      backupStatus = "Monitoring...";
    });

    return () => {
      unlistenStart.then(fn => fn());
      unlistenEnd.then(fn => fn());
    };
  });

  async function login() {
    try {
      authStatus = "Starting rclone authentication...";
      await invoke("rclone_login");
      authStatus = "Connected!";
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
    
    if (selected && Array.isArray(selected)) {
      selected.forEach(path => {
        if (!backupItems.find(item => item.path === path)) {
          backupItems.push({ path, isDirectory, enabled: true });
        }
      });
    } else if (selected && typeof selected === "string") {
      if (!backupItems.find(item => item.path === selected)) {
        backupItems.push({ path: selected, isDirectory, enabled: true });
      }
    }
  }

  async function startMonitoring() {
    const enabledPaths = backupItems.filter(i => i.enabled).map(i => i.path);
    if (enabledPaths.length === 0) return;
    
    try {
      backupStatus = "Monitoring...";
      await invoke("start_backup", { 
        paths: enabledPaths,
        remoteFolder 
      });
    } catch (error) {
      backupStatus = `Error: ${error}`;
    }
  }

  function removeItem(path: string) {
    backupItems = backupItems.filter(i => i.path !== path);
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
        {#if isSyncing}
          <div class="spinner"></div>
        {/if}
      </div>
      <p>Status: <strong>{authStatus}</strong></p>
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
      <h3>2. Local Items</h3>
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
              <div class="item-info">
                <span class="item-name">{item.path.split('/').pop() || item.path.split('\\').pop()}</span>
                <div class="item-path-container">
                  <span class="item-path">{item.path}</span>
                </div>
              </div>
              <button class="remove-btn" onclick={() => removeItem(item.path)}>×</button>
            </div>
          {/each}
        {/if}
      </div>

      <div class="footer">
        <p>Backup Status: <strong>{backupStatus}</strong></p>
        <button 
          class="primary-btn" 
          onclick={startMonitoring} 
          disabled={backupItems.filter(i => i.enabled).length === 0 || authStatus !== 'Connected!'}
        >
          Start Monitoring
        </button>
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

.spinner {
  width: 16px;
  height: 16px;
  border: 2px solid rgba(0,0,0,0.1);
  border-left-color: #007bff;
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.prefix {
  font-weight: 700;
  opacity: 0.6;
  padding: 6px 4px 6px 10px;
  background: #eee;
  border-radius: 6px 0 0 6px;
  border: 1px solid #ccc;
  border-right: none;
}

@media (prefers-color-scheme: dark) {
  .prefix {
    background: #3d3d3d;
    border-color: #4d4d4d;
  }
}

.row {
  display: flex;
  gap: 0px;
}

.row input {
  border-top-left-radius: 0;
  border-bottom-left-radius: 0;
  flex: 1;
}

.help-text {
  font-size: 0.75rem;
  opacity: 0.6;
  margin-top: 8px;
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
  max-height: 250px;
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
  border-bottom: 1px solid #eee;
  gap: 12px;
}

@media (prefers-color-scheme: dark) {
  .item-row {
    border-bottom-color: #3a3a3a;
  }
}

.item-row:last-child {
  border-bottom: none;
}

.item-info {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.item-name {
  font-weight: 500;
  font-size: 0.9rem;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.item-path-container {
  width: 100%;
  overflow-x: auto;
}

.item-path {
  font-size: 0.7rem;
  opacity: 0.6;
  white-space: nowrap;
}

.remove-btn {
  padding: 2px 8px;
  font-size: 1.2rem;
  color: #ff4d4f;
  background: transparent;
  border: none;
}

.remove-btn:hover {
  background: #fff1f0;
  border-radius: 4px;
}

.empty-msg {
  text-align: center;
  padding: 24px;
  opacity: 0.5;
  font-style: italic;
}

.footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-top: 1px solid #eee;
  padding-top: 16px;
  margin-top: 8px;
}

@media (prefers-color-scheme: dark) {
  .footer {
    border-top-color: #3f3f3f;
  }
}

.primary-btn {
  background-color: #007bff;
  color: white;
  border: none;
  font-weight: 600;
}

.primary-btn:hover {
  background-color: #0056b3;
}

.primary-btn:disabled {
  background-color: #ccc;
}

.danger-btn {
  color: #ff4d4f;
  border-color: #ffa39e;
}

.danger-btn:hover {
  background-color: #fff1f0;
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
