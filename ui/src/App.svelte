<script>
  import UploadPage from "./pages/UploadPage.svelte";
  import PreviewPage from "./pages/PreviewPage.svelte";

  let path = $state(window.location.pathname);

  window.addEventListener("popstate", () => {
    path = window.location.pathname;
  });

  let match = $derived.by(() => {
    const m = path.match(/^\/f\/([a-zA-Z0-9_-]+)$/);
    return m ? m[1] : null;
  });
</script>

{#if match}
  <PreviewPage id={match} />
{:else}
  <UploadPage />
{/if}
