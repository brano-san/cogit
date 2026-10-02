import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
// @ts-expect-error plain .mjs next to the other repo scripts
import { OWNERS, audit } from "../../../scripts/audit-controls.mjs";

/** The guard against native-looking controls (doc/06 "Controls"). It greps every .svelte
    file for raw `<button>` / `<input>` / `<select>` / `<textarea>` and fails when nothing
    gives one a look: no class or tag rule in the file's own <style>, no global rule
    (`.btn`, `.tick-box`, range, text inputs in a Dialog). A new control that
    should be the shared Button / Checkbox / Radio / Select fails here instead of reaching
    the screen as a bare element. */
interface Found {
  file: string;
  line: number;
  tag: string;
  type: string;
  classes: string[];
  styled: boolean;
}

const root = path.resolve(__dirname, "../../..");
const found: Found[] = audit(path.join(root, "frontend/src"));
const allow: Record<string, string> = JSON.parse(fs.readFileSync(path.join(root, "scripts/controls.allow.json"), "utf8"));
const allowed = (f: Found) => `frontend/src/${f.file}` in allow;

describe("raw controls", () => {
  it("every raw control has a look", () => {
    const bare = found.filter((f) => !f.styled && !allowed(f)).map((f) => `${f.file}:${f.line} <${f.tag}${f.type ? " " + f.type : ""}>`);
    expect(bare).toEqual([]);
  });

  it("selects, checkboxes and radios exist only in their shared component", () => {
    const stray = found
      .filter((f) => f.tag === "select" || (f.tag === "input" && (f.type === "checkbox" || f.type === "radio")))
      .filter((f) => !(OWNERS[f.tag === "select" ? "select" : f.type] ?? []).includes(f.file) && !f.classes.includes("tick-box"))
      .map((f) => `${f.file}:${f.line} <${f.tag} ${f.type}>`);
    expect(stray).toEqual([]);
  });

  it("every allowlist entry still points at a file", () => {
    const missing = Object.keys(allow).filter((key) => key !== "_doc" && !fs.existsSync(path.join(root, key)));
    expect(missing).toEqual([]);
  });
});

/** The small panel button is `class="btn sm"` (app.css), not a local copy of its box: copies
    drifted apart, each with its own disabled look. The files below predate the rule; one that
    switches to `.btn sm` leaves the list, a new copy fails here. */
const OLD_COPIES = new Set([
  "components/common/QueueNav.svelte",
  "components/investigate/OriginCard.svelte",
  "components/layout/KeymapEditor.svelte", // the key-capture field, mono
  "components/layout/Notifications.svelte",
  "components/layout/StateBanner.svelte",
  "components/repo-tree/RepositoryList.svelte", // the filter field, an input
  "components/repo-tree/WorktreeList.svelte",
  "components/solver/SolverWholeFile.svelte",
]);

describe("small buttons", () => {
  it("no component draws its own panel button", () => {
    const dir = path.join(root, "frontend/src/components");
    const copies: string[] = [];
    for (const file of fs.readdirSync(dir, { recursive: true, encoding: "utf8" })) {
      if (!file.endsWith(".svelte")) continue;
      const rel = `components/${file.replaceAll("\\", "/")}`;
      const style = /<style[^>]*>([\s\S]*?)<\/style>/.exec(fs.readFileSync(path.join(dir, file), "utf8"))?.[1] ?? "";
      for (const [, selector = "", body = ""] of style.matchAll(/([^{}]+)\{([^}]*)\}/g)) {
        const copy = /height:\s*var\(--h-button-sm\)/.test(body) && /background:\s*var\(--(surface|bg)-input\)/.test(body);
        if (copy && !/\binput\b/.test(selector) && !OLD_COPIES.has(rel)) copies.push(`${rel}: ${selector.trim()}`);
      }
    }
    expect(copies).toEqual([]);
  });

  it("every old copy is still one", () => {
    const gone = [...OLD_COPIES].filter((rel) => {
      const src = fs.readFileSync(path.join(root, "frontend/src", rel), "utf8");
      return !/height:\s*var\(--h-button-sm\)/.test(src);
    });
    expect(gone).toEqual([]);
  });
});
