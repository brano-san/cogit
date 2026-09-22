import type { AppInfo, OsInfo } from "$lib/ipc";
import type { UpdateOutcome } from "$lib/updates";

/** What the window shows beside `AppInfo`: things only the frontend knows. */
export interface Context {
  svelte: string;
  /** `navigator.language`: the system's, since the interface itself is English only. */
  locale: string;
  update: UpdateOutcome | null;
}

export interface AboutRow {
  label: string;
  value: string;
  href?: string;
  /** A muted marker after the value, such as `dirty`. */
  tag?: string;
  /** Cut in the middle to fit, with the whole path in the tooltip. */
  path?: boolean;
  title?: string;
  /** A link that runs an action in place of opening a page. */
  action?: "check";
}

export interface AboutGroup {
  title: string;
  rows: AboutRow[];
}

const INTERFACE_LANGUAGE = "English";

/** A build that was not stamped — a source tarball with no `.git` — says so. */
export function buildDate(epochSeconds: number): string {
  if (epochSeconds <= 0) return "unknown";
  return new Date(epochSeconds * 1000).toISOString().slice(0, 10);
}

function buildTime(epochSeconds: number): string {
  if (epochSeconds <= 0) return "unknown";
  return new Date(epochSeconds * 1000).toISOString().replace(/\.\d+Z$/, "Z");
}

export function shortRustc(rustc: string): string {
  return /^rustc (\S+)/.exec(rustc)?.[1] ?? rustc;
}

export function shortGit(git: string): string {
  return /^git version (\S+)/.exec(git)?.[1] ?? git;
}

/** `Windows 11 24H2 (build 26100), x86_64`; `full` adds the edition and the revision. */
export function osLabel(os: OsInfo, full: boolean): string {
  const words = [os.product];
  if (full && os.edition) words.push(os.edition);
  if (os.release) words.push(os.release);
  let label = words.join(" ");
  if (os.build) {
    label += ` (build ${os.build}${full && os.revision ? `.${os.revision}` : ""})`;
  } else if (os.kernel) {
    label += ` (kernel ${os.kernel})`;
  }
  return `${label}, ${os.arch}`;
}

function stamped(info: AppInfo): boolean {
  return info.commit !== "unknown" && info.commit !== "";
}

function commitRow(info: AppInfo): AboutRow {
  const row: AboutRow = { label: "Commit", value: info.commit };
  if (info.dirty) row.tag = "dirty";
  else if (stamped(info)) row.href = `${info.repository}/commit/${info.commit}`;
  return row;
}

export function updateRow(update: UpdateOutcome | null, repository: string): AboutRow {
  const label = "Updates";
  if (update === null) return { label, value: "Not checked", action: "check" };
  switch (update.kind) {
    case "none":
      return { label, value: "Up to date" };
    case "available":
      return {
        label,
        value: `${update.version} available`,
        href: `${repository}/releases/latest`,
      };
    case "failed":
      return { label, value: "Check failed", title: update.reason, action: "check" };
  }
}

export function aboutGroups(info: AppInfo, context: Context): AboutGroup[] {
  return [
    {
      title: "Application",
      rows: [
        { label: "Version", value: info.version },
        commitRow(info),
        { label: "Built", value: buildDate(info.builtAt) },
        updateRow(context.update, info.repository),
      ],
    },
    {
      title: "Environment",
      rows: [
        { label: "OS", value: osLabel(info.os, false) },
        { label: "Renderer", value: info.renderer },
        { label: "Git", value: shortGit(info.git) },
      ],
    },
    {
      title: "Built with",
      rows: [
        { label: "Rust", value: shortRustc(info.rustc) },
        { label: "Tauri", value: info.tauri },
        { label: "Svelte", value: context.svelte },
        { label: "gitoxide", value: info.gitLibrary },
      ],
    },
    {
      title: "Files",
      rows: [
        { label: "Log folder", value: info.logDir, path: true },
        { label: "Settings file", value: info.settingsPath, path: true },
      ],
    },
  ];
}

function displayLines(info: AppInfo): string[] {
  return [
    `Displays: ${info.displays.length}`,
    ...info.displays.map((display, index) => {
      const scale = display.scale === null ? "?" : `${Math.round(display.scale * 100)}%`;
      const primary = display.primary ? " (primary)" : "";
      return `Display ${index + 1}: ${display.width}x${display.height} at ${scale}${primary}`;
    }),
  ];
}

/** Everything the window shows, in full, and what it leaves out. Plain `key: value`
    lines, so the text pastes into an issue as it is. */
export function diagnosticsText(info: AppInfo, context: Context): string {
  const lines: [string, string][] = [
    ["Version", info.version],
    ["Commit", info.dirty ? `${info.commit} (dirty)` : info.commit],
    ["Built", buildTime(info.builtAt)],
    ["Build profile", info.debugBuild ? "debug" : "release"],
  ];
  if (context.update) lines.push(["Updates", updateRow(context.update, info.repository).value]);
  lines.push(
    ["OS", osLabel(info.os, true)],
    ["Renderer", info.renderer],
    ["Git", info.git],
    ["Rust", info.rustc],
    ["Tauri", info.tauri],
    ["Svelte", context.svelte],
    ["gitoxide", info.gitLibrary],
    ["Log folder", info.logDir],
    ["Log file", info.logPath],
    ["Settings file", info.settingsPath],
  );
  return [
    ...lines.map(([key, value]) => `${key}: ${value}`),
    ...displayLines(info),
    `Interface language: ${INTERFACE_LANGUAGE}`,
    `System locale: ${context.locale}`,
  ].join("\n");
}
