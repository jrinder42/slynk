<script lang="ts">
  let status = $state("Up to date");
</script>

<div class="popover minimalist">
  <header>
    <h1>Slynk</h1>
    <span data-testid="status" class="status">{status}</span>
  </header>
  <main>
    <p>All your local files are synced to Google Drive.</p>
  </main>
</div>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  }

  .popover {
    padding: 1rem;
    width: 280px;
    background-color: white;
    color: #333;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .minimalist {
    border-radius: 12px;
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid #eee;
    padding-bottom: 0.5rem;
  }

  h1 {
    font-size: 1rem;
    font-weight: 600;
    margin: 0;
  }

  .status {
    font-size: 0.75rem;
    color: #666;
    background: #f0f0f0;
    padding: 0.2rem 0.5rem;
    border-radius: 10px;
  }

  main p {
    font-size: 0.875rem;
    color: #444;
    margin: 0;
    line-height: 1.4;
  }
</style>
