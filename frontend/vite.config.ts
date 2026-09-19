// `vitest/config` rather than `vite`: it is the same helper widened with the `test`
// key, which plain Vite does not know about.
import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { fileURLToPath, URL } from "node:url";

// Tauri watches this port; changing it means changing `devUrl` in tauri.conf.json too.
const DEV_PORT = 1420;

export default defineConfig({
  plugins: [svelte()],

  resolve: {
    alias: {
      $lib: fileURLToPath(new URL("./src/lib", import.meta.url)),
      $components: fileURLToPath(new URL("./src/components", import.meta.url)),
      $stores: fileURLToPath(new URL("./src/stores", import.meta.url)),
    },
  },

  // Vite prints its own errors; Tauri needs a fixed port and a hard failure if it is taken.
  clearScreen: false,
  server: {
    port: DEV_PORT,
    strictPort: true,
    watch: {
      // Rust rebuilds are driven by cargo, not by Vite.
      ignored: ["**/src-tauri/**", "**/target/**"],
    },
  },

  build: {
    // Webview2 on Windows 11 and WebKit on Linux both handle modern output.
    target: "esnext",
    sourcemap: true,
    outDir: "dist",
    emptyOutDir: true,
  },

  test: {
    environment: "node",
    include: ["src/**/*.{test,spec}.ts"],
  },
});
