import { describe, expect, it } from "vitest";
import type { Branch, Tag } from "$lib/ipc";
import { filterRevisions, revisionOptions, stepOption } from "./revision-options";

const branch = (name: string, kind: "local" | "remote"): Branch =>
  ({ name, fullName: name, kind, oid: "a".repeat(40), isHead: false }) as Branch;
const tag = (name: string): Tag => ({ name, fullName: name, oid: "b".repeat(40), isAnnotated: false, pointsToCommit: true });

const branches = [branch("main", "local"), branch("dev", "local"), branch("origin/main", "remote"), branch("origin/HEAD", "remote")];

describe("revisionOptions", () => {
  it("lists HEAD and the selected commit first, then branches and tags in groups", () => {
    const options = revisionOptions({ branches, tags: [tag("v1")], selectedCommit: "c".repeat(40) });
    expect(options.map((o) => [o.group, o.label])).toEqual([
      ["", "HEAD"],
      ["", "Selected commit"],
      ["Local branches", "dev"],
      ["Local branches", "main"],
      ["Remote branches", "origin/main"],
      ["Tags", "v1"],
    ]);
    expect(options[1]!.value).toBe("c".repeat(40));
  });

  it("leaves out the selected commit when none is selected and the remote HEAD alias", () => {
    const labels = revisionOptions({ branches, tags: [] }).map((o) => o.label);
    expect(labels).not.toContain("Selected commit");
    expect(labels).not.toContain("origin/HEAD");
  });

  it("disables a branch held by another worktree, with where", () => {
    const options = revisionOptions({ branches, tags: [], special: false, held: new Map([["dev", "C:/wt/dev"]]) });
    const dev = options.find((o) => o.label === "dev")!;
    expect(dev.disabled).toBe(true);
    expect(dev.reason).toBe("Checked out in C:/wt/dev");
    expect(options.find((o) => o.label === "HEAD")).toBeUndefined();
  });

  it("disables a remote branch whose local twin exists when asked to create tracking branches", () => {
    const options = revisionOptions({ branches, tags: [], special: false, localTwins: true });
    const remote = options.find((o) => o.label === "origin/main")!;
    expect(remote.disabled).toBe(true);
    expect(remote.reason).toContain("main");
  });
});

describe("filterRevisions", () => {
  const options = revisionOptions({ branches, tags: [tag("v1"), tag("main-tag")] });

  it("matches case-insensitively on any part and keeps group headers of what is left", () => {
    const { rows } = filterRevisions(options, "MAIN");
    expect(rows.map((r) => (r.kind === "group" ? `# ${r.title}` : r.option.label))).toEqual([
      "# Local branches",
      "main",
      "# Remote branches",
      "origin/main",
      "# Tags",
      "main-tag",
    ]);
  });

  it("caps the rows and says how many are hidden", () => {
    const many = revisionOptions({
      branches: Array.from({ length: 500 }, (_, i) => branch(`b${i}`, "local")),
      tags: [],
    });
    const found = filterRevisions(many, "", 100);
    expect(found.options).toHaveLength(100);
    expect(found.hidden).toBe(401);
  });
});

describe("stepOption", () => {
  const options = [{ disabled: false }, { disabled: true }, { disabled: false }] as { disabled?: boolean }[];
  it("skips disabled entries and stops at the ends", () => {
    expect(stepOption(options as never, 0, 1)).toBe(2);
    expect(stepOption(options as never, 2, -1)).toBe(0);
    expect(stepOption(options as never, 2, 1)).toBe(2);
    expect(stepOption(options as never, -1, 1)).toBe(0);
  });
});
