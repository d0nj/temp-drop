<script>
  let { title = "TEMPDROP", id = null, statusText = "ONLINE" } = $props();
</script>

<header class="top-nav">
  <a href="/" class="brand-link">
    <span class="brand-icon">⚡</span>
    <span class="brand-title">{title}</span>
  </a>
  <div class="status-badge">
    {#if !id}
      <span class="pulse-dot"></span>
      <span class="status-text">{statusText}</span>
    {:else}
      <span class="status-text">{id}</span>
    {/if}
  </div>
</header>

<style>
  .top-nav {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 2px solid #262a36;
    padding-bottom: 12px;
    margin-bottom: 20px;
  }

  .brand-link {
    display: flex;
    align-items: center;
    gap: 8px;
    text-decoration: none;
  }

  .brand-icon {
    font-size: 18px;
  }

  .brand-title {
    font-family: "JetBrains Mono", monospace;
    font-weight: 700;
    font-size: 14px;
    letter-spacing: 0.08em;
    color: #00ff66;
  }

  .status-badge {
    display: flex;
    align-items: center;
    gap: 8px;
    background: #0d121d;
    border: 1px solid #262a36;
    padding: 4px 10px;
  }

  .pulse-dot {
    width: 6px;
    height: 6px;
    background-color: #00ff66;
    border-radius: 50%;
    box-shadow: 0 0 8px #00ff66;
  }

  .status-text {
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    color: #94a3b8;
    letter-spacing: 0.05em;
  }
</style>
