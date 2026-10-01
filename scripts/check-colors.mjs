#!/usr/bin/env node
// Fails on a color literal outside frontend/src/themes/. Colors live in the theme files
// (doc/06-design-system.md); components, CSS and canvas code use tokens only.
// Scans frontend/src/**/*.{svelte,ts,css,html} and frontend/*.html, skipping tests and the
// generated IPC bindings. An unavoidable literal goes in scripts/check-colors.allow as
// `path :: text that is on the line`, with a reason after `#`.
import { readFileSync, readdirSync, statSync, existsSync } from "node:fs";
import { join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(fileURLToPath(import.meta.url), "..", "..");
const frontend = join(root, "frontend");
const SKIP_DIRS = new Set(["node_modules", "dist", "themes"]);
const EXT = /\.(svelte|ts|css|html)$/;

const NAMED =
  "aliceblue antiquewhite aqua aquamarine azure beige bisque black blanchedalmond blue blueviolet brown burlywood cadetblue chartreuse chocolate coral cornflowerblue cornsilk crimson cyan darkblue darkcyan darkgoldenrod darkgray darkgreen darkgrey darkkhaki darkmagenta darkolivegreen darkorange darkorchid darkred darksalmon darkseagreen darkslateblue darkslategray darkslategrey darkturquoise darkviolet deeppink deepskyblue dimgray dimgrey dodgerblue firebrick floralwhite forestgreen fuchsia gainsboro ghostwhite gold goldenrod gray green greenyellow grey honeydew hotpink indianred indigo ivory khaki lavender lavenderblush lawngreen lemonchiffon lightblue lightcoral lightcyan lightgoldenrodyellow lightgray lightgreen lightgrey lightpink lightsalmon lightseagreen lightskyblue lightslategray lightslategrey lightsteelblue lightyellow lime limegreen linen magenta maroon mediumaquamarine mediumblue mediumorchid mediumpurple mediumseagreen mediumslateblue mediumspringgreen mediumturquoise mediumvioletred midnightblue mintcream mistyrose moccasin navajowhite navy oldlace olive olivedrab orange orangered orchid palegoldenrod palegreen paleturquoise palevioletred papayawhip peachpuff peru pink plum powderblue purple rebeccapurple red rosybrown royalblue saddlebrown salmon sandybrown seagreen seashell sienna silver skyblue slateblue slategray slategrey snow springgreen steelblue tan teal thistle tomato turquoise violet wheat white whitesmoke yellow yellowgreen".split(
    " ",
  );
const NAMED_RE = new RegExp(String.raw`(?<![\w$#-])(${NAMED.join("|")})(?![\w-])`, "i");

const HEX = /(?<![\w&/{#.-])#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})(?![\w-]|\s*[:=(])/gi;
const FUNC = /\b(rgb|rgba|hsl|hsla|hwb|lab|lch|oklab|oklch)\(/i;
const MIX = /color-mix\(.*\b(transparent|white|black)\b/i;
// `color: red`, `border-left: 1px solid red`, `--x: red`: a named color in a color-ish property.
const PROPERTY =
  /(?:^|[\s;{"'])(color|background(?:-color|-image)?|border(?:-[a-z]+)*|outline(?:-color)?|fill|stroke|stop-color|flood-color|box-shadow|text-shadow|caret-color|accent-color|text-decoration(?:-color)?|--[\w-]+)\s*:\s*([^;}{]*)/gi;
const ATTRIBUTE = /\b(fill|stroke|stop-color|flood-color|color)=["']\s*([a-z]+)\s*["']/gi;
const CANVAS = /\b(fillStyle|strokeStyle|shadowColor)\s*=\s*["']([a-z]+)["']/g;

function allowList() {
  const file = join(root, "scripts", "check-colors.allow");
  if (!existsSync(file)) return [];
  return readFileSync(file, "utf8")
    .split("\n")
    .map((line) => line.replace(/\s+#.*$/, "").trim())
    .filter((line) => line && !line.startsWith("#"))
    .map((line) => line.split(" :: ").map((part) => part.trim()));
}

function* files(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) {
      if (!SKIP_DIRS.has(name)) yield* files(path);
    } else if (EXT.test(name) && !name.endsWith(".test.ts") && !path.endsWith(join("ipc", "bindings.ts"))) {
      yield path;
    }
  }
}

/** The literal on a line, or null. Exported for the test. */
export function literalIn(line) {
  // Prose is not rendered.
  if (/^\s*(\/\/|\/\*|\*|<!--)/.test(line)) return null;
  for (const match of line.matchAll(HEX)) {
    // `#123` is an issue number or a count far more often than a color.
    if (/^\d{3,4}$/.test(match[1])) continue;
    return match[0];
  }
  const func = FUNC.exec(line);
  if (func) return func[0];
  const mix = MIX.exec(line);
  if (mix) return mix[0];
  for (const match of line.matchAll(PROPERTY)) {
    const value = match[2].replace(/var\([^)]*\)/g, "").replace(/url\([^)]*\)/g, "");
    const named = NAMED_RE.exec(value);
    if (named) return `${match[1]}: …${named[1]}`;
  }
  for (const match of line.matchAll(ATTRIBUTE)) if (NAMED.includes(match[2].toLowerCase())) return match[0];
  for (const match of line.matchAll(CANVAS)) if (NAMED.includes(match[2].toLowerCase())) return match[0];
  return null;
}

export function scan() {
  const allowed = allowList();
  const found = [];
  const targets = [
    ...files(join(frontend, "src")),
    ...readdirSync(frontend)
      .filter((name) => name.endsWith(".html"))
      .map((name) => join(frontend, name)),
  ];
  for (const path of targets) {
    const rel = relative(root, path).split(sep).join("/");
    readFileSync(path, "utf8")
      .split("\n")
      .forEach((line, index) => {
        const literal = literalIn(line);
        if (!literal) return;
        if (allowed.some(([file, text]) => file === rel && line.includes(text ?? ""))) return;
        found.push(`${rel}:${index + 1}: ${literal}   ${line.trim().slice(0, 100)}`);
      });
  }
  return found;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const found = scan();
  if (found.length) {
    console.error(`${found.length} color literal(s) outside frontend/src/themes/:\n${found.join("\n")}`);
    console.error("\nUse a token (var(--bg-panel)); to add one see doc/06-design-system.md.");
    process.exit(1);
  }
  console.log("check-colors: ok");
}
