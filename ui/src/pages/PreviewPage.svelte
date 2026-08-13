<script>
  import { onMount, onDestroy } from "svelte";
  import Header from "../components/Header.svelte";
  import Footer from "../components/Footer.svelte";
  import { api } from "../lib/api.js";
  import { formatBytes, formatCountdown, formatUTC } from "../lib/utils.js";
  import { downloadAndDecryptFile } from "../lib/downloader.js";
  import { importKeyFromString, decryptName } from "../lib/crypto.js";

  let { id } = $props();
  let meta = $state(null);
  let notFound = $state(false);
  let now = $state(Date.now() / 1000);
  let decrypting = $state(false);
  let decryptError = $state(null);
  let displayFilename = $state("Encrypted File");

  // Extract decryption key from URL hash (e.g. #key=1a2b3c...)
  let keyStr = $state(null);

  async function resolveFilename() {
    if (!meta) return;
    if (meta.name.startsWith("enc:")) {
      if (keyStr) {
        try {
          const key = await importKeyFromString(keyStr);
          displayFilename = await decryptName(meta.name.slice(4), key);
        } catch {
          displayFilename = "Encrypted Vault File";
        }
      } else {
        displayFilename = "Encrypted Vault File";
      }
    } else {
      displayFilename = meta.name;
    }
  }

  async function load() {
    try {
      meta = await api(`/api/uploads/${id}`);
      if (meta.downloads_left !== null && meta.downloads_left <= 0) {
        notFound = true;
        return;
      }
      notFound = false;
      await resolveFilename();
    } catch (e) {
      if (e.code === "not_found") notFound = true;
    }
  }

  function readHashKey() {
    const hash = location.hash;
    const match = hash.match(/#key=([a-fA-F0-9]+)/);
    if (match) {
      keyStr = match[1];
    }
  }

  let timer;
  onMount(() => {
    readHashKey();
    load();
    timer = setInterval(() => {
      now = Date.now() / 1000;
    }, 1000);
  });
  onDestroy(() => clearInterval(timer));

  let downloadProgress = $state(null);

  async function handleDownload() {
    if (!meta) return;
    if (!keyStr) {
      decryptError = "Decryption key missing from share link.";
      return;
    }
    decrypting = true;
    decryptError = null;
    downloadProgress = { received: 0, total: meta.size || 0 };
    try {
      await downloadAndDecryptFile({
        id,
        filename: displayFilename,
        keyStr,
        chunkSize: meta.chunk_size,
        onProgress: (received, total) => {
          downloadProgress = { received, total: total || meta?.size || 0 };
        },
      });
    } catch (e) {
      decryptError = "Decryption failed: " + (e.message || String(e));
    } finally {
      decrypting = false;
      downloadProgress = null;
    }
  }

  let downloadsLeft = $derived(meta?.downloads_left ?? null);
</script>

<div class="viewport-wrapper">
  <main class="page-container">
    <Header id={id} />

    <div class="brutalist-card">
      {#if notFound}
        <div class="not-found-panel">
          <div class="banner banner-error">
            <span class="banner-code">404 NOT FOUND</span>
            <span class="banner-msg">File reference expired, purged, or invalid.</span>
          </div>
          <p class="panel-desc">
            This file no longer exists. It may have exceeded its expiration duration or download limit.
          </p>
          <a class="btn btn-primary" href="/">UPLOAD NEW FILE</a>
        </div>
      {:else if !meta}
        <div class="loading-panel">
          <div class="custom-loader">
            <svg class="loader-svg" viewBox="0 0 50 50" width="44" height="44">
              <circle class="loader-track" cx="25" cy="25" r="18" fill="none" stroke="#1f2432" stroke-width="4" />
              <circle class="loader-spinner" cx="25" cy="25" r="18" fill="none" stroke="#00ff66" stroke-width="4" stroke-linecap="round" />
            </svg>
          </div>
          <p class="mono-text">Loading file details...</p>
        </div>
      {:else}
        <div class="manifest-header">
          <div class="manifest-tag">
            ENCRYPTED VAULT FILE
          </div>
          <h1 class="file-title" title={displayFilename}>{displayFilename}</h1>
          <div class="size-pill">{formatBytes(meta.size)}</div>
        </div>

        <div class="specs-table">
          <div class="spec-row">
            <span class="spec-label">SECURITY</span>
            <span class="spec-value mono accent-green">
              AES-256-GCM
            </span>
          </div>

          <div class="spec-row">
            <span class="spec-label">DECRYPTION KEY</span>
            <span class="spec-value mono" class:accent-green={keyStr} class:accent-red={!keyStr}>
              {keyStr ? "Present" : "Missing"}
            </span>
          </div>

          <div class="spec-row">
            <span class="spec-label">CREATED</span>
            <span class="spec-value mono">{formatUTC(meta.created_at)}</span>
          </div>

          {#if meta.expires_at}
            <div class="spec-row highlight-row">
              <span class="spec-label">AUTO DESTRUCT</span>
              <span class="spec-value mono accent-green">{formatCountdown(meta.expires_at, now)}</span>
            </div>
          {/if}

          {#if downloadsLeft != null}
            <div class="spec-row highlight-row">
              <span class="spec-label">DOWNLOADS REMAINING</span>
              <span class="spec-value mono accent-green">{downloadsLeft} of {meta.max_downloads}</span>
            </div>
          {/if}
        </div>

        {#if !keyStr}
          <div class="banner banner-error">
            <span class="banner-msg">Decryption key missing. Use the complete share link to access this file.</span>
          </div>
        {:else if decryptError}
          <div class="banner banner-error">
            <span class="banner-msg">{decryptError}</span>
          </div>
        {/if}

        {#if downloadProgress}
          <div class="progress-panel">
            <div class="progress-info">
              <span class="progress-label">DOWNLOADING</span>
              <span class="progress-bytes">
                {formatBytes(downloadProgress.received)} / {formatBytes(downloadProgress.total || meta.size)}
              </span>
            </div>
            <div class="bar-frame">
              <div
                class="bar-fill"
                style="transform: scaleX({downloadProgress.received / (downloadProgress.total || meta.size || 1)})"
              ></div>
            </div>
          </div>
        {/if}

        <div class="action-section">
          <button class="btn btn-download" onclick={handleDownload} disabled={decrypting || !keyStr}>
            {#if decrypting}
              DECRYPTING...
            {:else if keyStr}
              DOWNLOAD FILE
            {:else}
              KEY REQUIRED
            {/if}
          </button>
        </div>

        <div class="nav-footer">
          <a href="/" class="back-link">← Upload another file</a>
        </div>
      {/if}
    </div>

    <Footer />
  </main>
</div>

<style>
  :global(html, body) {
    margin: 0;
    padding: 0;
    min-height: 100%;
    background-color: #07080a;
  }

  :global(body) {
    font-family: "Familjen Grotesk", system-ui, -apple-system, sans-serif;
    color: #f1f5f9;
    -webkit-font-smoothing: antialiased;
  }

  .viewport-wrapper {
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px 16px;
    box-sizing: border-box;
  }

  .page-container {
    width: 100%;
    max-width: 540px;
  }

  .brutalist-card {
    background: #0c0d12;
    border: 2px solid #262a36;
    padding: 28px 32px;
    box-shadow: 8px 8px 0px #000000;
  }

  .manifest-header {
    margin-bottom: 20px;
    background: #08090e;
    border: 1px solid #262a36;
    padding: 16px 18px;
  }

  .manifest-tag {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    font-weight: 700;
    color: #00ff66;
    letter-spacing: 0.05em;
    margin-bottom: 4px;
  }

  .file-title {
    font-size: 20px;
    font-weight: 800;
    letter-spacing: -0.01em;
    margin: 0 0 8px 0;
    color: #ffffff;
    word-break: break-all;
  }

  .size-pill {
    display: inline-block;
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    font-weight: 700;
    background: #141722;
    border: 1px solid #262a36;
    color: #00ff66;
    padding: 3px 8px;
  }

  .specs-table {
    border: 2px solid #262a36;
    background: #08090e;
    margin-bottom: 20px;
  }

  .spec-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 16px;
    border-bottom: 1px solid #1f2432;
    font-size: 12px;
  }

  .spec-row:last-child {
    border-bottom: none;
  }

  .highlight-row {
    background: #06140c;
  }

  .spec-label {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: #8a8f9d;
    letter-spacing: 0.05em;
  }

  .spec-value {
    color: #ffffff;
    font-weight: 600;
  }

  .spec-value.mono {
    font-family: "JetBrains Mono", monospace;
  }

  .accent-green {
    color: #00ff66;
    font-weight: 700;
  }

  .accent-red {
    color: #ff2e4c;
    font-weight: 700;
  }

  .action-section {
    margin-bottom: 18px;
  }

  .btn {
    font-family: "Familjen Grotesk", sans-serif;
    font-size: 14px;
    font-weight: 700;
    padding: 14px 20px;
    border: 2px solid #ffffff;
    cursor: pointer;
    text-decoration: none;
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 4px 4px 0px #ffffff;
    transition: transform 0.1s ease, box-shadow 0.1s ease;
    width: 100%;
    box-sizing: border-box;
  }

  .btn:hover:not(:disabled) {
    transform: translate(-2px, -2px);
    box-shadow: 6px 6px 0px #ffffff;
  }

  .btn:active:not(:disabled) {
    transform: translate(2px, 2px);
    box-shadow: 2px 2px 0px #ffffff;
  }

  .btn-download {
    background: #00ff66;
    color: #000000;
    border-color: #00ff66;
    box-shadow: 4px 4px 0px #00aa44;
    font-size: 15px;
    letter-spacing: 0.03em;
  }

  .btn-download:hover:not(:disabled) {
    box-shadow: 6px 6px 0px #00aa44;
  }

  .btn-download:disabled {
    background: #1e2433;
    color: #64748b;
    border-color: #262a36;
    box-shadow: none;
    cursor: not-allowed;
    transform: none;
  }

  .btn-primary {
    background: #00ff66;
    color: #000000;
    border-color: #00ff66;
    box-shadow: 4px 4px 0px #00aa44;
  }

  .nav-footer {
    text-align: center;
    padding-top: 12px;
    border-top: 1px dashed #262a36;
  }

  .back-link {
    font-family: "JetBrains Mono", monospace;
    color: #8a8f9d;
    font-size: 11px;
    text-decoration: none;
    font-weight: 700;
  }

  .back-link:hover {
    color: #00ff66;
  }

  .not-found-panel {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .panel-desc {
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    color: #94a3b8;
    line-height: 1.5;
    margin: 0;
  }

  .loading-panel {
    text-align: center;
    padding: 40px 0;
  }

  .custom-loader {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    margin-bottom: 12px;
  }

  .loader-svg {
    animation: loader-rotate 1.2s linear infinite;
  }

  .loader-spinner {
    animation: loader-dash 1.5s ease-in-out infinite;
    transform-origin: center;
  }

  @keyframes loader-rotate {
    100% {
      transform: rotate(360deg);
    }
  }

  @keyframes loader-dash {
    0% {
      stroke-dasharray: 1, 150;
      stroke-dashoffset: 0;
    }
    50% {
      stroke-dasharray: 90, 150;
      stroke-dashoffset: -35;
    }
    100% {
      stroke-dasharray: 90, 150;
      stroke-dashoffset: -124;
    }
  }

  .mono-text {
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    color: #8a8f9d;
  }

  .banner {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 12px 14px;
    border: 2px solid;
    margin-bottom: 18px;
    font-family: "JetBrains Mono", monospace;
  }

  .banner-error {
    background: #1a0507;
    border-color: #ff2e4c;
  }

  .banner-error .banner-code {
    color: #ff2e4c;
    font-size: 10px;
    font-weight: 700;
  }

  .banner-error .banner-msg {
    color: #ffffff;
    font-size: 12px;
  }

  .progress-panel {
    border: 1px solid #00ff66;
    background: #04120a;
    padding: 12px 14px;
    margin-bottom: 20px;
  }

  .progress-info {
    display: flex;
    justify-content: space-between;
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    margin-bottom: 8px;
  }

  .progress-label {
    color: #00ff66;
    font-weight: 700;
  }

  .progress-bytes {
    color: #ffffff;
  }

  .bar-frame {
    height: 10px;
    background: #0d2618;
    border: 1px solid #00ff66;
    overflow: hidden;
  }

  .bar-fill {
    height: 100%;
    width: 100%;
    background: #00ff66;
    transform-origin: left;
    transition: transform 0.15s linear;
  }
</style>
