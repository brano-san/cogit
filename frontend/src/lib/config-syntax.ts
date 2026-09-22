/** One run of a git config line; `cls` is a Lezer token class, empty for plain text. */
export interface ConfigToken {
  text: string;
  cls: string;
}

const plain = (text: string): ConfigToken => ({ text, cls: "" });

/** Where an unquoted `#` or `;` starts a comment, or -1. */
function commentAt(text: string): number {
  let quoted = false;
  for (let at = 0; at < text.length; at++) {
    const char = text[at];
    if (char === "\\") at++;
    else if (char === '"') quoted = !quoted;
    else if (!quoted && (char === "#" || char === ";")) return at;
  }
  return -1;
}

function valueClass(value: string): string {
  if (/^(true|false|yes|no|on|off)$/i.test(value)) return "tok-bool";
  if (/^-?\d+[kmg]?$/i.test(value)) return "tok-number";
  return "tok-string";
}

function section(line: string, lead: string): ConfigToken[] {
  const body = line.slice(lead.length);
  const close = body.indexOf("]");
  const head = close === -1 ? body : body.slice(0, close);
  const quote = head.indexOf('"');
  const out: ConfigToken[] = [plain(lead)];
  if (quote === -1) out.push({ text: head, cls: "tok-typeName" });
  else {
    out.push({ text: head.slice(0, quote).trimEnd(), cls: "tok-typeName" });
    out.push(plain(head.slice(head.slice(0, quote).trimEnd().length, quote)));
    out.push({ text: head.slice(quote), cls: "tok-string" });
  }
  if (close !== -1) {
    out.push({ text: "]", cls: "tok-typeName" });
    out.push(...rest(body.slice(close + 1)));
  }
  return out;
}

/** Whatever follows a section header or a value: spaces, then maybe a comment. */
function rest(tail: string): ConfigToken[] {
  const at = commentAt(tail);
  if (at === -1) return tail === "" ? [] : [plain(tail)];
  return [plain(tail.slice(0, at)), { text: tail.slice(at), cls: "tok-comment" }];
}

export function tokenizeConfigLine(line: string): ConfigToken[] {
  const lead = /^\s*/.exec(line)![0];
  const body = line.slice(lead.length);
  if (body === "") return line === "" ? [] : [plain(line)];
  if (body[0] === "#" || body[0] === ";") return [plain(lead), { text: body, cls: "tok-comment" }];
  if (body[0] === "[") return section(line, lead).filter((token) => token.text !== "");

  const equals = body.indexOf("=");
  const key = equals === -1 ? body.trimEnd() : body.slice(0, equals).trimEnd();
  const out: ConfigToken[] = [plain(lead), { text: key, cls: "tok-propertyName" }];
  if (equals === -1) {
    out.push(plain(body.slice(key.length)));
    return out.filter((token) => token.text !== "");
  }

  out.push(plain(body.slice(key.length, equals + 1)));
  const after = body.slice(equals + 1);
  const space = /^\s*/.exec(after)![0];
  const valueText = after.slice(space.length);
  const cut = commentAt(valueText);
  const raw = cut === -1 ? valueText : valueText.slice(0, cut);
  const value = raw.trimEnd();
  out.push(plain(space));
  if (value !== "") out.push({ text: value, cls: valueClass(value) });
  out.push(...rest(valueText.slice(value.length)));
  return out.filter((token) => token.text !== "");
}
