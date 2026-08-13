import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Version comes from the GitHub release tag (GITHUB_REF_NAME, e.g. "v0.1.1")
// during CI builds; local dev builds fall back to "dev".
const version = (process.env.GITHUB_REF_NAME ?? "dev").replace(/^v/, "");

export default defineConfig({
  plugins: [svelte()],
  build: { outDir: "dist" },
  define: {
    __APP_VERSION__: JSON.stringify(version),
  },
});
