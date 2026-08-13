<script>
  import Header from "../components/Header.svelte";
  import Footer from "../components/Footer.svelte";
  import { uploadFile } from "../lib/uploader.js";
  import { formatBytes } from "../lib/utils.js";
  import { generateQRCodeSVG } from "../lib/qr.js";

  let file = $state(null);
  let dragging = $state(false);
  let ttlPreset = $state("86400");
  let customHours = $state(168);
  let maxDownloads = $state(3);
  let mode = $state("ttl");
  let progress = $state(null); // {sent, total}
  let busy = $state(false);
  let result = $state(null); // {url, id, keyStr}
  let error = $state(null);
  let copied = $state(false);
  let qrSvg = $state("");

  $effect(() => {
    if (result?.url) {
      const fullUrl = location.origin + result.url;
      generateQRCodeSVG(fullUrl).then((svg) => {
        qrSvg = svg;
      });
    } else {
      qrSvg = "";
    }
  });

  function onFiles(files) {
    if (!files || !files.length) return;
    file = files[0];
    result = null;
    error = null;
  }

  function dropHandler(e) {
    e.preventDefault();
    dragging = false;
    onFiles(e.dataTransfer?.files);
  }

  async function go() {
    if (!file || busy) return;
    busy = true;
    error = null;
    progress = { sent: 0, total: file.size };
    const ttlSeconds =
      mode === "ttl"
        ? ttlPreset === "custom"
          ? customHours * 3600
          : parseInt(ttlPreset, 10)
        : null;
    const md = mode === "downloads" ? parseInt(maxDownloads, 10) : null;
    try {
      result = await uploadFile({
        file,
        ttlSeconds,
        maxDownloads: md,
        onProgress: (sent, total) => {
          progress = { sent, total };
        },
      });
    } catch (e) {
      error = e.message || String(e);
    } finally {
      busy = false;
    }
  }

  function copyLink() {
    if (result) {
      const fullUrl = location.origin + result.url;
      navigator.clipboard.writeText(fullUrl);
      copied = true;
      setTimeout(() => {
        copied = false;
      }, 2000);
    }
  }

  function reset() {
    file = null;
    result = null;
    error = null;
    progress = null;
  }
</script>

<svelte:window
  onpaste={(e) => onFiles(e.clipboardData?.files)}
  ondragover={(e) => {
    e.preventDefault();
    dragging = true;
  }}
  ondragleave={() => (dragging = false)}
  ondrop={dropHandler}
/>

<div class="viewport-wrapper">
  <main class="page-container">
    <Header />

    <div class="brutalist-card">
      <div class="hero-block">
        <h1 class="hero-header-tag">EPHEMERAL FILE VAULT</h1>
        <p class="hero-sub">
          Encrypted, self-destructing file transfer.
        </p>
      </div>

      {#if result}
        <div class="result-panel">
          <div class="banner banner-success">
            <span class="banner-msg">FILE SECURED IN VAULT</span>
          </div>

          <div class="result-details">
            <label class="section-label" for="share-url-input">SHARE LINK</label>
            <div class="input-action-group">
              <input
                id="share-url-input"
                type="text"
                readonly
                value={location.origin + result.url}
                class="mono-input"
                onclick={(e) => e.currentTarget.select()}
              />
              <button class="btn btn-emerald" onclick={copyLink}>
                {copied ? "COPIED" : "COPY LINK"}
              </button>
            </div>
          </div>

          {#if qrSvg}
            <div class="qr-container">
              <div class="qr-box">
                {@html qrSvg}
              </div>
              <span class="qr-caption">Scan to open share link on mobile</span>
            </div>
          {/if}

          <div class="button-grid">
            <a class="btn btn-outline" href={result.url}>VIEW PAGE</a>
            <button class="btn btn-ghost" onclick={reset}>NEW UPLOAD</button>
          </div>
        </div>
      {:else}
        <label class="drop-zone" class:is-dragging={dragging}>
          <input
            type="file"
            hidden
            onchange={(e) => onFiles(e.currentTarget.files)}
          />
          {#if file}
            <div class="file-manifest">
              <span class="manifest-tag">SELECTED</span>
              <div class="manifest-info">
                <span class="manifest-name">{file.name}</span>
                <span class="manifest-bytes">{formatBytes(file.size)}</span>
              </div>
              <span class="manifest-action">Change file</span>
            </div>
          {:else}
            <div class="drop-prompt">
              <div class="drop-icon-box">⬆</div>
              <p class="drop-primary">
                Drop file here or <span class="accent-underline">browse</span>
              </p>
              <p class="drop-secondary">Clipboard paste supported</p>
            </div>
          {/if}
        </label>

        <div class="rules-box">
          <div class="box-header">
            <span class="box-title">EXPIRATION</span>
          </div>

          <div class="mode-tabs">
            <button
              type="button"
              class="tab-btn"
              class:is-active={mode === "ttl"}
              onclick={() => (mode = "ttl")}
            >
              TIME LIMIT
            </button>
            <button
              type="button"
              class="tab-btn"
              class:is-active={mode === "downloads"}
              onclick={() => (mode = "downloads")}
            >
              DOWNLOAD LIMIT
            </button>
          </div>

          <div class="config-content">
            {#if mode === "ttl"}
              <div class="control-row">
                <label class="field-label" for="ttl-select">DURATION</label>
                <select id="ttl-select" bind:value={ttlPreset} class="mono-select">
                  <option value="3600">1 Hour</option>
                  <option value="86400">24 Hours</option>
                  <option value="604800">7 Days</option>
                  <option value="custom">Custom</option>
                </select>
              </div>
              {#if ttlPreset === "custom"}
                <div class="control-row inline-row">
                  <label class="field-label" for="custom-hours-input">HOURS</label>
                  <input
                    id="custom-hours-input"
                    type="number"
                    min="1"
                    bind:value={customHours}
                    class="mono-input short-input"
                  />
                </div>
              {/if}
            {:else}
              <div class="control-row inline-row">
                <label class="field-label" for="max-downloads-input">MAX DOWNLOADS</label>
                <input
                  id="max-downloads-input"
                  type="number"
                  min="1"
                  bind:value={maxDownloads}
                  class="mono-input short-input"
                />
              </div>
            {/if}
          </div>
        </div>

        {#if progress}
          <div class="progress-panel">
            <div class="progress-info">
              <span class="progress-label">UPLOADING</span>
              <span class="progress-bytes">
                {formatBytes(progress.sent)} / {formatBytes(progress.total)}
              </span>
            </div>
            <div class="bar-frame">
              <div
                class="bar-fill"
                style="transform: scaleX({progress.sent / (progress.total || 1)})"
              ></div>
            </div>
          </div>
        {/if}

        {#if error}
          <div class="banner banner-error">
            <span class="banner-msg">{error}</span>
          </div>
        {/if}

        <button
          class="btn btn-primary-action"
          onclick={go}
          disabled={!file || busy}
        >
          {#if busy}
            UPLOADING...
          {:else if !file}
            SELECT FILE
          {:else}
            UPLOAD FILE ➔
          {/if}
        </button>
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

  .hero-block {
    margin-bottom: 20px;
    background: #08090e;
    border: 1px solid #262a36;
    padding: 14px 16px;
    text-align: center;
  }

  .hero-header-tag {
    font-family: "Familjen Grotesk", sans-serif;
    font-size: 18px;
    font-weight: 800;
    color: #ffffff;
    letter-spacing: -0.01em;
    margin: 0 0 4px 0;
  }

  .hero-sub {
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    color: #94a3b8;
    margin: 0;
  }

  .drop-zone {
    display: block;
    border: 2px dashed #3a3f52;
    background: #07080c;
    padding: 32px 20px;
    text-align: center;
    cursor: pointer;
    margin-bottom: 20px;
    transition: border-color 0.15s ease, background-color 0.15s ease;
  }

  .drop-zone:hover,
  .drop-zone.is-dragging {
    border-color: #00ff66;
    background: #051a10;
  }

  .drop-icon-box {
    font-family: "JetBrains Mono", monospace;
    font-size: 24px;
    font-weight: 700;
    color: #00ff66;
    margin-bottom: 8px;
  }

  .drop-primary {
    font-size: 15px;
    font-weight: 700;
    color: #ffffff;
    margin: 0 0 4px 0;
  }

  .accent-underline {
    color: #00ff66;
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  .drop-secondary {
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    color: #64748b;
    margin: 0;
  }

  .file-manifest {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }

  .manifest-tag {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    background: #00ff66;
    color: #000000;
    padding: 2px 6px;
    font-weight: 700;
  }

  .manifest-info {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .manifest-name {
    font-size: 15px;
    font-weight: 700;
    color: #ffffff;
    word-break: break-all;
  }

  .manifest-bytes {
    font-family: "JetBrains Mono", monospace;
    color: #00ff66;
    font-size: 12px;
  }

  .manifest-action {
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    color: #8a8f9d;
    text-decoration: underline;
    margin-top: 2px;
  }

  .rules-box {
    border: 2px solid #262a36;
    background: #08090e;
    margin-bottom: 20px;
  }

  .box-header {
    background: #141722;
    border-bottom: 1px solid #262a36;
    padding: 8px 12px;
  }

  .box-title {
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    font-weight: 700;
    color: #00ff66;
    letter-spacing: 0.05em;
  }

  .mode-tabs {
    display: grid;
    grid-template-columns: 1fr 1fr;
    border-bottom: 1px solid #262a36;
  }

  .tab-btn {
    background: transparent;
    border: none;
    border-right: 1px solid #262a36;
    color: #8a8f9d;
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    font-weight: 700;
    padding: 10px;
    cursor: pointer;
    transition: background 0.1s ease, color 0.1s ease;
  }

  .tab-btn:last-child {
    border-right: none;
  }

  .tab-btn.is-active {
    background: #00ff66;
    color: #000000;
  }

  .config-content {
    padding: 14px 16px;
  }

  .control-row {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .inline-row {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
  }

  .field-label {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: #cbd5e1;
    letter-spacing: 0.04em;
  }

  .mono-select,
  .mono-input {
    font-family: "JetBrains Mono", monospace;
    background: #0f121d;
    border: 1px solid #262a36;
    color: #ffffff;
    padding: 8px 12px;
    font-size: 12px;
    outline: none;
    width: 100%;
    box-sizing: border-box;
  }

  .mono-select:focus,
  .mono-input:focus {
    border-color: #00ff66;
  }

  .short-input {
    width: 110px;
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

  .banner {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 12px 14px;
    border: 2px solid;
    margin-bottom: 20px;
    font-family: "JetBrains Mono", monospace;
  }

  .banner-success {
    background: #041a0e;
    border-color: #00ff66;
  }

  .banner-success .banner-msg {
    color: #ffffff;
    font-size: 12px;
    font-weight: 700;
  }

  .banner-error {
    background: #1a0507;
    border-color: #ff2e4c;
  }

  .banner-error .banner-msg {
    color: #ffffff;
    font-size: 12px;
  }

  .btn {
    font-family: "Familjen Grotesk", sans-serif;
    font-size: 14px;
    font-weight: 700;
    padding: 14px 20px;
    border: 2px solid #ffffff;
    cursor: pointer;
    text-decoration: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    box-shadow: 4px 4px 0px #ffffff;
    transition: transform 0.1s ease, box-shadow 0.1s ease;
  }

  .btn:hover:not(:disabled) {
    transform: translate(-2px, -2px);
    box-shadow: 6px 6px 0px #ffffff;
  }

  .btn:active:not(:disabled) {
    transform: translate(2px, 2px);
    box-shadow: 2px 2px 0px #ffffff;
  }

  .btn-primary-action {
    width: 100%;
    background: #00ff66;
    color: #000000;
    border-color: #00ff66;
    box-shadow: 4px 4px 0px #00aa44;
    font-size: 15px;
    letter-spacing: 0.03em;
  }

  .btn-primary-action:hover:not(:disabled) {
    box-shadow: 6px 6px 0px #00aa44;
  }

  .btn-primary-action:disabled {
    background: #1e2433;
    color: #64748b;
    border-color: #262a36;
    box-shadow: none;
    cursor: not-allowed;
    transform: none;
  }

  .btn-emerald {
    background: #00ff66;
    color: #000000;
    border-color: #00ff66;
    box-shadow: 3px 3px 0px #00aa44;
  }

  .btn-outline {
    background: transparent;
    color: #ffffff;
    border-color: #ffffff;
  }

  .btn-ghost {
    background: transparent;
    color: #8a8f9d;
    border-color: #262a36;
    box-shadow: none;
  }

  .btn-ghost:hover {
    color: #ffffff;
    border-color: #8a8f9d;
    box-shadow: 3px 3px 0px #8a8f9d;
  }

  .result-panel {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .section-label {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: #8a8f9d;
    letter-spacing: 0.05em;
    display: block;
    margin-bottom: 6px;
  }

  .input-action-group {
    display: flex;
    gap: 10px;
  }

  .input-action-group .mono-input {
    flex: 1;
  }

  .button-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .qr-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    background: #08090e;
    border: 1px solid #262a36;
    padding: 16px;
  }

  .qr-box {
    width: 160px;
    height: 160px;
    background: #08090e;
    border: 1px solid #262a36;
    padding: 8px;
    box-sizing: border-box;
  }

  .qr-box :global(svg) {
    width: 100%;
    height: 100%;
    display: block;
  }

  .qr-caption {
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    color: #8a8f9d;
  }
</style>
