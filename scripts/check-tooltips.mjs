// One tooltip per window (TooltipLayer): it answers `title` and `data-tip` and keeps the native
// one away for the element and every titled element around it. What it cannot reach is a
// tooltip the browser draws itself: an SVG <title>. This fails on one in frontend/src.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const root = join(import.meta.dirname, "..", "frontend", "src");
const found = [];
const walk = (dir) => {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) walk(path);
    else if (name.endsWith(".svelte")) {
      readFileSync(path, "utf8")
        .split("\n")
        .forEach((line, at) => {
          const code = line.trim();
          // Prose about the page's `<title>` is no SVG element.
          if (code.startsWith("//") || code.startsWith("*") || code.includes("`<title")) return;
          if (/<title[\s>]/.test(code)) found.push(`${relative(root, path)}:${at + 1}`);
        });
    }
  }
};
walk(root);
if (found.length > 0) {
  console.error(
    `SVG <title> draws a native tooltip beside the app's own; use title= or data-tip= on the element:\n${found.join("\n")}`,
  );
  process.exit(1);
}
console.log("check-tooltips: ok");
