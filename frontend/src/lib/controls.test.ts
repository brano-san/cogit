import fs from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
// @ts-expect-error plain .mjs next to the other repo scripts
import { OWNERS, audit } from "../../../scripts/audit-controls.mjs";

/** The guard against native-looking controls (doc/06 "Controls"). It greps every .svelte
    file for raw `<button>` / `<input>` / `<select>` / `<textarea>` and fails when nothing
    gives one a look: no class or tag rule in the file's own <style>, no global rule
    (`.btn` in a Dialog, `.tick-box`, range, text inputs in a Dialog). A new control that
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
