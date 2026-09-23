import { defineConfig } from "vitest/config";
import type { Plugin } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { fileURLToPath, URL } from "node:url";
import { resolve } from "node:path";
import fs from "node:fs";
import { createRequire } from "node:module";
import {
  THIRD_PARTY_FILE,
  bundledRoots,
  describePackage,
  renderPackages,
  type BundledPackage,
} from "./src/lib/third-party";

const DEV_PORT = 1420;

function thirdPartyLicences(): Plugin {
  return {
    name: "cogit-third-party-licences",
    apply: "build",
    generateBundle(_options, bundle) {
      const modules: [string, number][] = [];
      for (const item of Object.values(bundle)) {
        if (item.type !== "chunk") continue;
        for (const [id, rendered] of Object.entries(item.modules)) {
          modules.push([id, rendered.renderedLength]);
        }
      }
      const manifest = (root: string) => JSON.parse(fs.readFileSync(`${root}/package.json`, "utf8"));
      const packages = bundledRoots(modules)
        .map((root) => describePackage(manifest(root)))
        .filter((entry): entry is BundledPackage => entry !== null);
      const source = renderPackages(packages);
      this.emitFile({ type: "asset", fileName: THIRD_PARTY_FILE, source });
    },
  };
}

// The About window names the toolkit it is drawn with; the backend cannot see it.
// Resolved rather than guessed: the install is hoisted to the repository root.
const svelteVersion = JSON.parse(
  fs.readFileSync(createRequire(import.meta.url).resolve("svelte/package.json"), "utf8"),
).version as string;

export default defineConfig({
  plugins: [svelte(), thirdPartyLicences()],

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
      // One entry point per child window, so each survives a webview reload (T2.5).
      input: {
        main: resolve(fileURLToPath(new URL(".", import.meta.url)), "index.html"),
        compare: resolve(fileURLToPath(new URL(".", import.meta.url)), "compare.html"),
        merge: resolve(fileURLToPath(new URL(".", import.meta.url)), "merge.html"),
        investigate: resolve(fileURLToPath(new URL(".", import.meta.url)), "investigate.html"),
        blame: resolve(fileURLToPath(new URL(".", import.meta.url)), "blame.html"),
      },
    },
  },

  test: {
    environment: "node",
    include: ["src/**/*.{test,spec}.ts"],
  },
});
