import { describe, expect, it } from "vitest";
import type { RepoSetting } from "./ipc/remote-ops";
import {
  FIELDS,
  TABS,
  changes,
  effective,
  initialDrafts,
  isOn,
  normalise,
  origin,
  separatorProblem,
  storage,
  type SettingField,
} from "./repo-settings";

function field(key: string): SettingField {
  const found = FIELDS.find((entry) => entry.key === key);
  if (!found) throw new Error(`${key} is not a field`);
  return found;
}

/** What `git_engine::REPO_SETTING_KEYS` offers, in its order. */
const BACKEND_KEYS = [
  "user.name",
  "user.email",
  "pull.rebase",
  "fetch.prune",
  "fetch.recurseSubmodules",
  "submodule.recurse",
  "cogit.initNewSubmodules",
  "push.default",
  "push.autoSetupRemote",
  "push.followTags",
  "commit.gpgSign",
  "tag.gpgSign",
  "user.signingKey",
  "gpg.format",
  "i18n.commitEncoding",
  "i18n.logOutputEncoding",
  "cogit.tagGroupSeparator",
];

describe("repository settings", () => {
  it("offers the tabs the task names, in its order", () => {
    expect(TABS.map(([, title]) => title)).toEqual([
      "User",
      "Fetch and Pull",
      "Push",
      "Signing",
      "Encoding",
      "Tag-Grouping",
    ]);
  });

  it("has a field for every key the backend reads and writes, and no other", () => {
    expect([...FIELDS.map((entry) => entry.key)].sort()).toEqual([...BACKEND_KEYS].sort());
  });

  it("puts every tab to use", () => {
    for (const [tab] of TABS) expect(FIELDS.some((entry) => entry.tab === tab), tab).toBe(true);
  });

  it("says for each option where it is kept", () => {
    expect(storage(field("user.name"))).toBe("user.name in this repository's .git/config");
    expect(storage(field("cogit.tagGroupSeparator"))).toContain("Cogit's own key cogit.tagGroupSeparator");
  });

  it("groups tags at a slash unless told otherwise", () => {
    const separator = field("cogit.tagGroupSeparator");
    expect(effective(separator, null, null)).toBe("/");
    expect(effective(separator, null, "-")).toBe("-");
    expect(effective(separator, ".", "-")).toBe(".");
  });

  it("reads git's booleans in all their spellings", () => {
    const control = field("fetch.prune").control;
    for (const value of ["true", "yes", "on", "1", "", "TRUE"]) expect(isOn(value, control), value).toBe(true);
    for (const value of ["false", "no", "off", "0"]) expect(isOn(value, control), value).toBe(false);
  });

  it("ticks submodule fetching only for true, not for on-demand", () => {
    const control = field("fetch.recurseSubmodules").control;
    expect(isOn("true", control)).toBe(true);
    expect(isOn("on-demand", control)).toBe(false);
  });

  it("shows where the value comes from", () => {
    const name = field("user.name");
    expect(origin(name, "Jane", "Global Jane")).toBe("Set in this repository");
    expect(origin(name, null, "Global Jane")).toBe("Not set here — inherited: Global Jane");
    expect(origin(name, null, null)).toBe("Not set anywhere");
    expect(origin(field("push.default"), null, null)).toBe("Not set anywhere — default: simple");
  });

  it("treats cleared text as not set, so the inherited value applies", () => {
    expect(normalise(field("user.email"), "  ")).toBeNull();
    expect(normalise(field("user.email"), "a@b.c")).toBe("a@b.c");
    expect(normalise(field("fetch.prune"), "false")).toBe("false");
  });

  /** The Branches panel reads an empty separator as "no tag folders" (agent "branches"). */
  it("keeps an empty separator, which turns tag folders off", () => {
    const separator = field("cogit.tagGroupSeparator");
    expect(normalise(separator, "")).toBe("");
    expect(effective(separator, "", "/")).toBe("");
    expect(separatorProblem("")).toBeNull();
  });

  it("writes only what changed, and removes what was made not set", () => {
    const read: RepoSetting[] = [
      { key: "user.name", local: "Jane", inherited: "Global" },
      { key: "push.default", local: null, inherited: null },
      { key: "fetch.prune", local: "true", inherited: null },
    ];
    const drafts = new Map(initialDrafts(read));
    expect(changes(read, drafts)).toEqual([]);

    drafts.set("user.name", null);
    drafts.set("push.default", "current");
    expect(changes(read, drafts)).toEqual([
      { key: "user.name", value: null },
      { key: "push.default", value: "current" },
    ]);
  });

  it("refuses a separator a tag name could not hold", () => {
    expect(separatorProblem("/")).toBeNull();
    expect(separatorProblem(null)).toBeNull();
    expect(separatorProblem(" ")).not.toBeNull();
    expect(separatorProblem(":")).not.toBeNull();
  });
});
