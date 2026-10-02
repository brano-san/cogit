// Static audit of raw form controls: lists the ones nothing in their file's <style> (or the
// global `.btn` / `.tick-box` rules) gives a look to. Used by frontend/src/lib/controls.test.ts.
import fs from "node:fs";
import path from "node:path";

const TAGS = ["button", "select", "input", "textarea"];
const GLOBAL_CLASSES = new Set(["tick-box"]);

function walk(dir, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walk(p, out);
    else if (p.endsWith(".svelte")) out.push(p);
  }
  return out;
}

/** The opening tag starting at `i` (after `<tag`), through the matching `>` outside braces/quotes. */
function openingTag(src, i) {
  let depth = 0, q = "";
  for (let j = i; j < src.length; j++) {
    const c = src[j];
    if (q) { if (c === q && depth === 0) q = ""; continue; }
    if (c === "{") depth++;
    else if (c === "}") depth--;
    else if ((c === '"' || c === "'") && depth === 0) q = c;
    else if (c === ">" && depth === 0) return src.slice(i, j);
  }
  return src.slice(i);
}

/** Where each control kind may be written raw: the shared component that owns it. */
export const OWNERS = { select: ["components/common/Select.svelte"], checkbox: ["components/common/Checkbox.svelte"], radio: ["components/common/Radio.svelte"] };

export function audit(root) {
  const found = [];
  for (const file of walk(root)) {
    const src = fs.readFileSync(file, "utf8");
    const style = (src.match(/<style[^>]*>([\s\S]*?)<\/style>/) ?? [, ""])[1];
    const selectors = [...style.replace(/\/\*[\s\S]*?\*\//g, "").matchAll(/([^{}]+)\{/g)].map((m) => m[1]);
    const blank = (t) => t.replace(/[^\n]/g, " ");
    const markup = src.replace(/<style[\s\S]*?<\/style>/, blank).replace(/<script[\s\S]*?<\/script>/g, blank);
    const re = new RegExp(String.raw`<(${TAGS.join("|")})(?=[\s>/])`, "g");
    for (const m of markup.matchAll(re)) {
      const tag = m[1];
      const attrs = openingTag(markup, m.index + m[0].length);
      const classes = new Set();
      for (const c of attrs.matchAll(/\bclass="([^"]*)"/g)) for (const t of c[1].split(/\s+/)) if (/^[\w-]+$/.test(t)) classes.add(t);
      for (const c of attrs.matchAll(/\bclass:([\w-]+)/g)) classes.add(c[1]);
      const type = (attrs.match(/\btype="(\w+)"/) ?? [, ""])[1];
      const line = markup.slice(0, m.index).split("\n").length;
      const tagSel = new RegExp(String.raw`(^|[\s>+~,)])${tag}\b`);
      // Global looks: app.css styles every range; Dialog.svelte styles text-like inputs inside it.
      const inDialog = /<(Dialog|TemplateDialog)/.test(markup) && ["text", "search", "password", "number", "email"].includes(type);
      const styled =
        type === "range" ||
        inDialog ||
        [...classes].some((c) => GLOBAL_CLASSES.has(c) || selectors.some((s) => s.includes("." + c))) ||
        selectors.some((s) => tagSel.test(s.trim()) || tagSel.test(s)) ||
        /\bid="/.test(attrs) && selectors.some((s) => /#[\w-]+/.test(s));
      // `.btn` is the global push button in app.css.
      const btnGlobal = classes.has("btn");
      found.push({ file: path.relative(root, file).replaceAll("\\", "/"), line, tag, type, classes: [...classes], styled: styled || btnGlobal, btnGlobal });
    }
  }
  return found;
}

if (process.argv[1] && import.meta.url.endsWith(path.basename(process.argv[1]))) {
  const all = audit(path.resolve("frontend/src"));
  for (const f of all.filter((x) => !x.styled)) console.log(`${f.file}:${f.line} <${f.tag}${f.type ? " " + f.type : ""}> ${f.classes.join(".")}`);
  console.log(`${all.length} controls, ${all.filter((x) => !x.styled).length} unstyled`);
}
