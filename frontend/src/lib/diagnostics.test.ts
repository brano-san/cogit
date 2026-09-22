import { describe, expect, it } from "vitest";
import type { AppInfo } from "$lib/ipc";
import { buildDate, diagnosticsText, versionRows } from "./diagnostics";

const info: AppInfo = {
  version: "0.1.0",
  logPath: "C:\\Users\\a\\cogit.log",
  debugBuild: false,
  commit: "4aec5e6f1b2c",
  builtAt: 1_758_499_200,
  rustc: "rustc 1.98.1 (abcdef 2026-01-01)",
  tauri: "2.11.5",
  webview: "141.0.3537.85",
  git: "git version 2.51.0.windows.1",
  os: "windows x86_64",
};

describe("versionRows", () => {
  it("names every part a bug report needs", () => {
    const labels = versionRows(info, "5.57.1").map(([label]) => label);
    expect(labels).toEqual([
      "Cogit",
      "Commit",
      "Built",
      "Platform",
      "WebView2",
      "Rust",
      "Tauri",
      "Svelte",
      "Git",
    ]);
  });

  it("marks a debug build so a slow one is not reported as a release", () => {
    expect(versionRows({ ...info, debugBuild: true }, "5.57.1")[0]![1]).toContain("debug");
    expect(versionRows(info, "5.57.1")[0]![1]).not.toContain("debug");
  });

  it("says so rather than showing a blank when the build was not stamped", () => {
    const rows = versionRows({ ...info, commit: "unknown", builtAt: 0 }, "5.57.1");
    expect(rows.find(([label]) => label === "Commit")?.[1]).toBe("unknown");
    expect(rows.find(([label]) => label === "Built")?.[1]).toBe("unknown");
  });
});

describe("buildDate", () => {
  it("reads as a date, not as a number of seconds", () => {
    expect(buildDate(1_758_499_200)).toMatch(/2025/);
  });

  it("has nothing to show for an unstamped build", () => {
    expect(buildDate(0)).toBe("unknown");
  });
});

describe("diagnosticsText", () => {
  it("is one label-value pair per line, ready to paste into an issue", () => {
    const lines = diagnosticsText(info, "5.57.1").split("\n");
    expect(lines[0]).toBe("Cogit: 0.1.0");
    expect(lines).toContain("Git: git version 2.51.0.windows.1");
  });

  it("carries the log path, which is the next thing anyone asks for", () => {
    expect(diagnosticsText(info, "5.57.1")).toContain(info.logPath);
  });
});
