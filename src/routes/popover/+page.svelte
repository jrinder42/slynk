<script lang="ts">
  import TrayPopover from "$lib/components/TrayPopover.svelte";
</script>

<div class="popover-container">
  <TrayPopover />
</div>

<style>
  :global(body) {
    background-color: transparent !important;
    margin: 0;
    padding: 0;
    overflow: hidden;
  }

  .popover-container {
    background: transparent;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    height: 100vh;
  }
</style>
