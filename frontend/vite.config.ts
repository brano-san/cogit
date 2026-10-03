import { defineConfig } from "vitest/config";
import type { Plugin } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { fileURLToPath, URL } from "node:url";
import { resolve } from "node:path";
import fs from "node:fs";
import { createRequire } from "node:module";
import { THEME_CACHE_KEY } from "./src/lib/theme-cache";
import {
  THIRD_PARTY_FILE,
  bundledRoots,
  describePackage,
  renderPackages,
  type BundledPackage,
} from "./src/lib/third-party";

const DEV_PORT = 1420;

// Before the page has a stylesheet or a script: the last theme's background and text, so a
// window never opens white and a failed script is still readable. Read from themes/, so no
// color literal lives here.
function bootTheme(): Plugin {
  return {
    name: "cogit-boot-theme",
    transformIndexHtml() {
      const themes = { light: "light", lightGrey: "light-gray", darkGrey: "dark-gray", dark: "dark" };
      const css = Object.entries(themes)
        .map(([id, file]) => {
          const { tokens, family } = JSON.parse(fs.readFileSync(fileURLToPath(new URL(`./src/themes/${file}.json`, import.meta.url)), "utf8"));
          return `:root[data-theme="${id}"]{color-scheme:${family};--boot-bg:${tokens["bg.app"]};--boot-fg:${tokens["fg.primary"]}}`;
        })
        .join("");
      const script = `try{var t=localStorage.getItem("${THEME_CACHE_KEY}");if(t)document.documentElement.dataset.theme=t}catch(e){}`;
      return [
        { tag: "style", children: css, injectTo: "head" },
        { tag: "script", children: script, injectTo: "head" },
      ];
    },
  };
}

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
  plugins: [svelte(), thirdPartyLicences(), bootTheme()],

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
    // Tauri embeds every file of dist in the executable: source maps would add ~1.7 MB of
    // brotli to a binary nobody debugs through them. `COGIT_SOURCEMAP=1` brings them back.
    sourcemap: process.env.COGIT_SOURCEMAP === "1",
    outDir: "dist",
    emptyOutDir: true,
    rollupOptions: {
      // One entry point per child window, so each survives a webview reload (T2.5).
      input: {
        main: resolve(fileURLToPath(new URL(".", import.meta.url)), "index.html"),
        compare: resolve(fileURLToPath(new URL(".", import.meta.url)), "compare.html"),
        solver: resolve(fileURLToPath(new URL(".", import.meta.url)), "solver.html"),
        investigate: resolve(fileURLToPath(new URL(".", import.meta.url)), "investigate.html"),
        blame: resolve(fileURLToPath(new URL(".", import.meta.url)), "blame.html"),
        commit: resolve(fileURLToPath(new URL(".", import.meta.url)), "commit.html"),
        errors: resolve(fileURLToPath(new URL(".", import.meta.url)), "errors.html"),
      },
    },
  },

  test: {
    projects: [
      {
        extends: true,
        test: { name: "node", environment: "node", include: ["src/**/*.{test,spec}.ts"], exclude: ["**/*.svelte.test.ts"] },
      },
      {
        // Node, transformed for the client: Svelte compiles `$effect` to a no-op for the
        // server, and store tests that run effects need the real one.
        extends: true,
        resolve: { conditions: ["browser"] },
        test: {
          name: "runes",
          environment: "./src/test-env-runes.ts",
          include: ["src/**/*.svelte.test.ts"],
          server: { deps: { inline: [/svelte/] } },
        },
      },
    ],
  },
});
