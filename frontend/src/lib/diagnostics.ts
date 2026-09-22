import type { AppInfo } from "$lib/ipc";

/** A build that was not stamped — a source tarball with no `.git` — says so. */
export function buildDate(epochSeconds: number): string {
  if (epochSeconds <= 0) return "unknown";
  return new Date(epochSeconds * 1000).toISOString().slice(0, 10);
}

/** What the About window lists, and what Copy Diagnostics writes: the same rows, so a
    pasted report and the window a user is reading never disagree. */
export function versionRows(info: AppInfo, svelte: string): [string, string][] {
  return [
    ["Cogit", info.debugBuild ? `${info.version} (debug build)` : info.version],
    ["Commit", info.commit],
    ["Built", buildDate(info.builtAt)],
    ["Platform", info.os],
    ["WebView2", info.webview],
    ["Rust", info.rustc],
    ["Tauri", info.tauri],
    ["Svelte", svelte],
    ["Git", info.git],
  ];
}

export function diagnosticsText(info: AppInfo, svelte: string): string {
  const rows = versionRows(info, svelte).map(([label, value]) => `${label}: ${value}`);
  return [...rows, `Log: ${info.logPath}`].join("\n");
}
