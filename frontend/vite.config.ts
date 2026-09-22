import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { fileURLToPath, URL } from "node:url";
import { resolve } from "node:path";
import fs from "node:fs";
import { createRequire } from "node:module";

const DEV_PORT = 1420;

// The About window names the toolkit it is drawn with; the backend cannot see it.
// Resolved rather than guessed: the install is hoisted to the repository root.
const svelteVersion = JSON.parse(
  fs.readFileSync(createRequire(import.meta.url).resolve("svelte/package.json"), "utf8"),
).version as string;

export default defineConfig({
  plugins: [svelte()],

  define: {
    __SVELTE_VERSION__: JSON.stringify(svelteVersion),
  },

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
    // The About window shows the application icon, and it lives beside the Rust crate
    // that ships it rather than as a second copy under the frontend.
    fs: { allow: [".."] },
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
        merge: resolve(fileURLToPath(new URL(".", import.meta.url)), "merge.html"),
      },
    },
  },

  test: {
    environment: "node",
    include: ["src/**/*.{test,spec}.ts"],
  },
});
