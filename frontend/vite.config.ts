import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { fileURLToPath, URL } from "node:url";
import { resolve } from "node:path";

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

  clearScreen: false,
  server: {
    port: DEV_PORT,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**", "**/target/**"],
    },
  },

  build: {
    target: "esnext",
    sourcemap: true,
    outDir: "dist",
    emptyOutDir: true,
    rollupOptions: {
      // A second entry point, so the compare window survives a webview reload (T2.5).
      input: {
        main: resolve(fileURLToPath(new URL(".", import.meta.url)), "index.html"),
        compare: resolve(fileURLToPath(new URL(".", import.meta.url)), "compare.html"),
      },
    },
  },

  test: {
    environment: "node",
    include: ["src/**/*.{test,spec}.ts"],
  },
});
