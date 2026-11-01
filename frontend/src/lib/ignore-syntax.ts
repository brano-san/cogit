import type { ConfigToken } from "./config-syntax";

export function tokenizeIgnoreLine(line: string): ConfigToken[] {
  if (line === "") return [];
  if (line.startsWith("#")) return [{ text: line, cls: "tok-comment" }];
  const out: ConfigToken[] = [];
  let plain = "";
  const push = (text: string, cls: string) => {
    if (plain) out.push({ text: plain, cls: "" });
    plain = "";
    out.push({ text, cls });
  };
  let at = 0;
  if (line.startsWith("!")) {
    push("!", "tok-keyword");
    at = 1;
  }
  while (at < line.length) {
    const char = line[at] ?? "";
    if (char === "\\" && at + 1 < line.length) {
      push(line.slice(at, at + 2), "tok-number");
      at += 2;
    } else if (char === "*") {
      const run = line[at + 1] === "*" ? "**" : "*";
      push(run, "tok-atom");
      at += run.length;
    } else if (char === "?") {
      push("?", "tok-atom");
      at += 1;
    } else if (char === "[" && line.indexOf("]", at + 1) !== -1) {
      const end = line.indexOf("]", at + 1) + 1;
      push(line.slice(at, end), "tok-string");
      at = end;
    } else {
      plain += char;
      at += 1;
    }
  }
  if (plain) out.push({ text: plain, cls: "" });
  return out;
}
