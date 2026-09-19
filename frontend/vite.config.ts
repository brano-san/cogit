import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { fileURLToPath, URL } from "node:url";

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
  },

  test: {
    environment: "node",
    include: ["src/**/*.{test,spec}.ts"],
  },
});
