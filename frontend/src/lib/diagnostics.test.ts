import { describe, expect, it } from "vitest";
import type { AppInfo } from "$lib/ipc";
import {
  aboutGroups,
  buildDate,
  diagnosticsText,
  osLabel,
  shortGit,
  shortRustc,
  updateRow,
  type AboutRow,
  type Context,
} from "./diagnostics";

const info: AppInfo = {
  version: "0.1.0",
  debugBuild: false,
  commit: "4aec5e6f1b2c",
  dirty: false,
  builtAt: 1_758_499_200,
  repository: "https://github.com/brano-san/cogit",
  os: {
    product: "Windows 11",
    edition: "Home",
    release: "24H2",
    build: "26100",
    revision: "4946",
    kernel: null,
    arch: "x86_64",
  },
  renderer: "WebView2 153.0.4234.48",
  git: "git version 2.51.0.windows.1",
  rustc: "rustc 1.98.1 (48a229cea 2026-09-01)",
  tauri: "2.11.5",
  gitLibrary: "0.87.1",
  logPath: "C:\\Users\\a\\logs\\cogit-2026-09-23_10-00-00.log",
  logDir: "C:\\Users\\a\\logs",
  settingsPath: "C:\\Users\\a\\Roaming\\settings.json",
  displays: [
    { name: "\\\\.\\DISPLAY1", width: 2560, height: 1440, scale: 1.5, primary: true },
    { name: null, width: 1920, height: 1080, scale: 1, primary: false },
  ],
};

const context: Context = { svelte: "5.57.1", locale: "ru-RU", update: null };

function row(label: string, source = info, ctx = context): AboutRow {
  const found = aboutGroups(source, ctx)
    .flatMap((group) => group.rows)
    .find((candidate) => candidate.label === label);
  if (!found) throw new Error(`no row ${label}`);
  return found;
}

describe("aboutGroups", () => {
  it("splits the window into four named groups", () => {
    expect(aboutGroups(info, context).map((group) => group.title)).toEqual([
      "Application",
      "Environment",
      "Built with",
      "Files",
    ]);
  });

  it("puts each row in its group, the version labelled Version rather than Cogit", () => {
    const labels = aboutGroups(info, context).map((group) => group.rows.map((r) => r.label));
    expect(labels).toEqual([
      ["Version", "Commit", "Built", "Updates"],
      ["OS", "Renderer", "Git"],
      ["Rust", "Tauri", "Svelte", "gitoxide"],
      ["Log folder", "Settings file"],
    ]);
  });

  it("shows every version in the short form", () => {
    expect(row("Version").value).toBe("0.1.0");
    expect(row("Rust").value).toBe("1.98.1");
    expect(row("Git").value).toBe("2.51.0.windows.1");
    expect(row("Tauri").value).toBe("2.11.5");
    expect(row("Svelte").value).toBe("5.57.1");
    expect(row("gitoxide").value).toBe("0.87.1");
    expect(row("Built").value).toBe("2025-09-22");
  });

  it("keeps the build profile out of the window: it belongs to diagnostics", () => {
    expect(row("Version", { ...info, debugBuild: true }).value).toBe("0.1.0");
  });

  it("names the OS with its release and build, and the renderer with its engine", () => {
    expect(row("OS").value).toBe("Windows 11 24H2 (build 26100), x86_64");
    expect(row("Renderer").value).toBe("WebView2 153.0.4234.48");
  });

  it("links a clean commit to its page on GitHub", () => {
    const commit = row("Commit");
    expect(commit.href).toBe("https://github.com/brano-san/cogit/commit/4aec5e6f1b2c");
    expect(commit.tag).toBeUndefined();
  });

  it("marks a build from uncommitted code dirty and does not link it", () => {
    const commit = row("Commit", { ...info, dirty: true });
    expect(commit.value).toBe("4aec5e6f1b2c");
    expect(commit.tag).toBe("dirty");
    expect(commit.href).toBeUndefined();
  });

  it("links nothing when the build was not stamped", () => {
    const stamped = { ...info, commit: "unknown", builtAt: 0 };
    expect(row("Commit", stamped).href).toBeUndefined();
    expect(row("Commit", stamped).value).toBe("unknown");
    expect(row("Built", stamped).value).toBe("unknown");
  });

  it("marks the paths, which are cut in the middle rather than wrapped", () => {
    expect(row("Log folder")).toMatchObject({ value: info.logDir, path: true });
    expect(row("Settings file")).toMatchObject({ value: info.settingsPath, path: true });
    expect(row("OS").path).toBeUndefined();
  });
});

describe("updateRow", () => {
  it("says so when no check has run", () => {
    expect(updateRow(null, info.repository)).toMatchObject({
      value: "Not checked",
      action: "check",
    });
  });

  it("reports an up-to-date build", () => {
    expect(updateRow({ kind: "none" }, info.repository)).toMatchObject({ value: "Up to date" });
  });

  it("links a newer release", () => {
    const found = updateRow({ kind: "available", version: "0.2.0", notes: "" }, info.repository);
    expect(found.value).toBe("0.2.0 available");
    expect(found.href).toBe("https://github.com/brano-san/cogit/releases/latest");
  });

  it("offers a second try after a failed check", () => {
    const failed = updateRow({ kind: "failed", reason: "offline" }, info.repository);
    expect(failed).toMatchObject({ value: "Check failed", title: "offline", action: "check" });
  });
});

describe("osLabel", () => {
  it("is short in the window and complete in diagnostics", () => {
    expect(osLabel(info.os, false)).toBe("Windows 11 24H2 (build 26100), x86_64");
    expect(osLabel(info.os, true)).toBe("Windows 11 Home 24H2 (build 26100.4946), x86_64");
  });

  it("names the kernel where there is no build number", () => {
    const linux = {
      product: "Ubuntu 24.04",
      edition: null,
      release: null,
      build: null,
      revision: null,
      kernel: "6.8.0-48-generic",
      arch: "x86_64",
    };
    expect(osLabel(linux, false)).toBe("Ubuntu 24.04 (kernel 6.8.0-48-generic), x86_64");
  });

  it("leaves out what the system did not report", () => {
    const bare = { ...info.os, edition: null, release: null, build: null, revision: null };
    expect(osLabel(bare, true)).toBe("Windows 11, x86_64");
  });
});

describe("short versions", () => {
  it("keep the number of rustc and git, and pass anything else through", () => {
    expect(shortRustc("rustc 1.98.1 (48a229cea 2026-09-01)")).toBe("1.98.1");
    expect(shortRustc("unknown")).toBe("unknown");
    expect(shortGit("git version 2.51.0.windows.1")).toBe("2.51.0.windows.1");
    expect(shortGit("not found")).toBe("not found");
  });
});

describe("buildDate", () => {
  it("reads as a date, not as a number of seconds", () => {
    expect(buildDate(1_758_499_200)).toBe("2025-09-22");
  });

  it("has nothing to show for an unstamped build", () => {
    expect(buildDate(0)).toBe("unknown");
  });
});

describe("diagnosticsText", () => {
  const lines = diagnosticsText(info, context).split("\n");

  it("is one key: value pair per line, ready to paste into an issue", () => {
    for (const line of lines) expect(line).toMatch(/^[A-Za-z][\w -]*: \S/);
    expect(lines[0]).toBe("Version: 0.1.0");
  });

  it("carries what the window shows in its full form", () => {
    expect(lines).toContain("Commit: 4aec5e6f1b2c");
    expect(lines).toContain("Built: 2025-09-22T00:00:00Z");
    expect(lines).toContain("OS: Windows 11 Home 24H2 (build 26100.4946), x86_64");
    expect(lines).toContain("Renderer: WebView2 153.0.4234.48");
    expect(lines).toContain("Git: git version 2.51.0.windows.1");
    expect(lines).toContain("Rust: rustc 1.98.1 (48a229cea 2026-09-01)");
    expect(lines).toContain("gitoxide: 0.87.1");
    expect(lines).toContain(`Log folder: ${info.logDir}`);
    expect(lines).toContain(`Settings file: ${info.settingsPath}`);
  });

  it("adds what the window leaves out", () => {
    expect(lines).toContain("Build profile: release");
    expect(lines).toContain(`Log file: ${info.logPath}`);
    expect(lines).toContain("Displays: 2");
    expect(lines).toContain("Display 1: 2560x1440 at 150% (primary)");
    expect(lines).toContain("Display 2: 1920x1080 at 100%");
    expect(lines).toContain("Interface language: English");
    expect(lines).toContain("System locale: ru-RU");
  });

  it("names a debug build and a dirty tree", () => {
    const text = diagnosticsText({ ...info, debugBuild: true, dirty: true }, context);
    expect(text).toContain("Build profile: debug");
    expect(text).toContain("Commit: 4aec5e6f1b2c (dirty)");
  });

  it("reports the update check only once one has run", () => {
    expect(lines.some((line) => line.startsWith("Updates:"))).toBe(false);
    const checked = diagnosticsText(info, { ...context, update: { kind: "none" } });
    expect(checked).toContain("Updates: Up to date");
  });
});
