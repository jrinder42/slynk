<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  let name = $state("");
  let greetMsg = $state("");
  let authStatus = $state("Not connected");
  let rcloneVersion = $state("Checking...");

  async function greet(event: Event) {
    event.preventDefault();
    greetMsg = await invoke("greet", { name });
  }

  async function login() {
    try {
      authStatus = "Starting rclone authentication...";
      await invoke("rclone_login");
      authStatus = "Connected!";
    } catch (error) {
      authStatus = `Error: ${error}`;
    }
  }

  async function checkRclone() {
    try {
      rcloneVersion = await invoke("test_rclone");
    } catch (error) {
      rcloneVersion = `Error: ${error}`;
    }
  }

  $effect(() => {
    checkRclone();
  });
</script>

<main class="container">
  <h1>slynk</h1>

  <div class="content">
    <div class="auth-section">
      <p style="font-size: 0.8rem; opacity: 0.7;">{rcloneVersion}</p>
      <p>Status: {authStatus}</p>
      <button onclick={login} disabled={authStatus === 'Connected!'}>
        Connect to Google Drive
      </button>
    </div>

    <form class="row" onsubmit={greet}>
      <input id="greet-input" placeholder="Enter a name..." bind:value={name} />
      <button type="submit">Greet</button>
    </form>
    <p>{greetMsg}</p>
  </div>
</main>

<style>
.container {
  height: 100vh;
  margin: 0;
  padding: 32px;
  display: flex;
  flex-direction: column;
  background-color: #f6f6f6;
  box-sizing: border-box;
}

@media (prefers-color-scheme: dark) {
  .container {
    color: #f6f6f6;
    background-color: #2f2f2f;
  }
}

h1 {
  font-size: 2rem;
  margin: 0 0 24px 0;
  text-align: center;
}

.auth-section {
  background: white;
  padding: 16px;
  border-radius: 8px;
  margin-bottom: 24px;
  border: 1px solid #ddd;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

@media (prefers-color-scheme: dark) {
  .auth-section {
    background: #3d3d3d;
    border-color: #4d4d4d;
  }
}

.content {
  flex: 1;
}

.row {
  display: flex;
  gap: 8px;
}

input,
button {
  border-radius: 6px;
  border: 1px solid #ccc;
  padding: 6px 10px;
  font-size: 0.9em;
  font-family: inherit;
  outline: none;
}

button {
  cursor: pointer;
  background-color: #ffffff;
  transition: all 0.2s;
}

button:hover {
  background-color: #f0f0f0;
}

#greet-input {
  flex: 1;
}

@media (prefers-color-scheme: dark) {
  input,
  button {
    color: #ffffff;
    background-color: #3d3d3d;
    border-color: #4d4d4d;
  }
  button:hover {
    background-color: #4d4d4d;
  }
}
</style>
