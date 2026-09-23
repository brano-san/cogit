import type { RepoSetting, RepoSettingChange } from "./ipc/remote-ops";

/** Repository ▸ Settings (#42): the tabs, the options and where each one is kept. Every
    option lives in the repository's `.git/config`; `cogit.*` keys are Cogit's own there. */

export type SettingTab = "user" | "fetch" | "push" | "signing" | "encoding" | "tags";

export const TABS: readonly (readonly [SettingTab, string])[] = [
  ["user", "User"],
  ["fetch", "Fetch and Pull"],
  ["push", "Push"],
  ["signing", "Signing"],
  ["encoding", "Encoding"],
  ["tags", "Tag-Grouping"],
];

export type Control =
  | { kind: "text"; placeholder?: string }
  /** `on`/`off` are what a tick writes; git's booleans are read in all their spellings. */
  | { kind: "bool"; on: string; off: string }
  | { kind: "choice"; options: readonly (readonly [string, string])[] };

export interface SettingField {
  key: string;
  tab: SettingTab;
  label: string;
  control: Control;
  /** What applies when no config file sets the key: git's default, or Cogit's. */
  fallback: string;
  hint?: string;
  /** Set when an empty value means something of its own rather than "not set here". */
  empty?: string;
}

const BOOL: Control = { kind: "bool", on: "true", off: "false" };

export const FIELDS: readonly SettingField[] = [
  { key: "user.name", tab: "user", label: "Name", control: { kind: "text" }, fallback: "" },
  { key: "user.email", tab: "user", label: "Email", control: { kind: "text" }, fallback: "" },
  {
    key: "pull.rebase",
    tab: "fetch",
    label: "When pulling",
    control: {
      kind: "choice",
      options: [
        ["false", "Merge fetched remote changes"],
        ["true", "Rebase local branch onto fetched changes"],
      ],
    },
    fallback: "false",
    hint: "How git pull joins diverged branches. Cogit's own Pull fast-forwards only.",
  },
  {
    key: "fetch.prune",
    tab: "fetch",
    label: "Prune obsolete remote tracked branches",
    control: BOOL,
    fallback: "false",
    hint: "Cogit's Fetch always prunes; Pull and git on the command line follow this.",
  },
  {
    key: "fetch.recurseSubmodules",
    tab: "fetch",
    label: "Always fetch new commits, tags and branches from submodules",
    control: { kind: "bool", on: "true", off: "on-demand" },
    fallback: "on-demand",
    hint: "Unticked, git fetches a submodule only when the parent records a commit it lacks.",
  },
  {
    key: "submodule.recurse",
    tab: "fetch",
    label: "Update registered submodules",
    control: BOOL,
    fallback: "false",
    hint: "Pull and checkout also move initialised submodules to the recorded commit.",
  },
  {
    key: "cogit.initNewSubmodules",
    tab: "fetch",
    label: "Initialize new submodules",
    control: BOOL,
    fallback: "false",
    hint: "After a Pull, Cogit checks out submodules the pull brought in.",
  },
  {
    key: "push.default",
    tab: "push",
    label: "Push without a refspec pushes",
    control: {
      kind: "choice",
      options: [
        ["simple", "The current branch to its upstream of the same name (simple)"],
        ["current", "The current branch to a branch of the same name (current)"],
        ["upstream", "The current branch to its upstream (upstream)"],
        ["matching", "All branches with a namesake on the remote (matching)"],
        ["nothing", "Nothing — a refspec is required (nothing)"],
      ],
    },
    fallback: "simple",
  },
  {
    key: "push.autoSetupRemote",
    tab: "push",
    label: "Set the upstream on the first push of a new branch",
    control: BOOL,
    fallback: "false",
  },
  {
    key: "push.followTags",
    tab: "push",
    label: "Push annotated tags that point at pushed commits",
    control: BOOL,
    fallback: "false",
  },
  { key: "commit.gpgSign", tab: "signing", label: "Sign commits", control: BOOL, fallback: "false" },
  { key: "tag.gpgSign", tab: "signing", label: "Sign annotated tags", control: BOOL, fallback: "false" },
  {
    key: "gpg.format",
    tab: "signing",
    label: "Signature format",
    control: {
      kind: "choice",
      options: [
        ["openpgp", "OpenPGP (gpg)"],
        ["ssh", "SSH key"],
        ["x509", "X.509 (gpgsm)"],
      ],
    },
    fallback: "openpgp",
  },
  {
    key: "user.signingKey",
    tab: "signing",
    label: "Signing key",
    control: { kind: "text", placeholder: "Key ID, or the path of an SSH key" },
    fallback: "",
    hint: "Empty: git picks the key from your name and email.",
  },
  {
    key: "i18n.commitEncoding",
    tab: "encoding",
    label: "Commit messages",
    control: { kind: "text", placeholder: "UTF-8" },
    fallback: "UTF-8",
    hint: "The encoding git records in new commits. Cogit sends messages as UTF-8: change it only for tools that write in another.",
  },
  {
    key: "i18n.logOutputEncoding",
    tab: "encoding",
    label: "Log output",
    control: { kind: "text", placeholder: "UTF-8" },
    fallback: "UTF-8",
    hint: "What git log converts messages to on the command line. Cogit reads commits itself.",
  },
  {
    key: "cogit.tagGroupSeparator",
    tab: "tags",
    label: "Group separator",
    control: { kind: "text", placeholder: "/" },
    fallback: "/",
    hint: "Tags are grouped into folders at this character in the Branches panel: release/1.0 goes into release/. Set it empty to show tags without folders.",
    empty: "(empty — tags are not grouped)",
  },
];

/** Where the dialog says the option is kept. */
export function storage(field: SettingField): string {
  return field.key.startsWith("cogit.")
    ? `Cogit's own key ${field.key} in this repository's .git/config; git ignores it`
    : `${field.key} in this repository's .git/config`;
}

/** An unset key in the repository: `null`. */
export type Drafts = ReadonlyMap<string, string | null>;

export function initialDrafts(settings: readonly RepoSetting[]): Drafts {
  return new Map(settings.map((entry) => [entry.key, entry.local]));
}

const TRUE = new Set(["true", "yes", "on", "1", ""]);

/** git's reading: a key with no value at all is `true`. */
export function isOn(value: string, control: Control): boolean {
  if (control.kind !== "bool") return false;
  const lower = value.trim().toLowerCase();
  return lower === control.on || (control.on === "true" && TRUE.has(lower));
}

export function effective(field: SettingField, draft: string | null, inherited: string | null): string {
  return draft ?? inherited ?? field.fallback;
}

/** Empty text means "not set here", so the inherited value applies again — unless the
    field gives empty a meaning of its own. */
export function normalise(field: SettingField, value: string): string | null {
  if (field.control.kind !== "text" || value.trim() !== "") return value;
  return field.empty === undefined ? null : "";
}

/** The line under each option: where its value comes from now. */
export function origin(field: SettingField, draft: string | null, inherited: string | null): string {
  if (draft !== null) return "Set in this repository";
  if (inherited !== null) return `Not set here — inherited: ${inherited === "" ? "(empty)" : inherited}`;
  return field.fallback === "" ? "Not set anywhere" : `Not set anywhere — default: ${field.fallback}`;
}

/** Only what differs from what was read; a key made "not set" is removed. */
export function changes(settings: readonly RepoSetting[], drafts: Drafts): RepoSettingChange[] {
  const out: RepoSettingChange[] = [];
  for (const entry of settings) {
    if (!drafts.has(entry.key)) continue;
    const draft = drafts.get(entry.key) ?? null;
    if (draft !== entry.local) out.push({ key: entry.key, value: draft });
  }
  return out;
}

/** The separator must be one or more characters that can sit inside a tag name. */
export function separatorProblem(value: string | null): string | null {
  if (value === null) return null;
  if (/\s/.test(value)) return "A separator cannot contain spaces";
  if (/[~^:?*[\\]/.test(value)) return "Git does not allow this character in tag names";
  return null;
}
