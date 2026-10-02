import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { binName, foreign, osOf, primary } from "./platform";

describe("platform", () => {
  it("reads the system from navigator.platform", () => {
    expect(osOf("Win32")).toBe("windows");
    expect(osOf("MacIntel")).toBe("mac");
    expect(osOf("Linux x86_64")).toBe("linux");
    expect(osOf("")).toBe("linux");
  });

  it("names the bin the way the system does", () => {
    expect(binName("windows")).toBe("Recycle Bin");
    expect(binName("mac")).toBe("Trash");
    expect(binName("linux")).toBe("Trash");
  });
});

const held = (ctrlKey: boolean, metaKey: boolean) => ({ ctrlKey, metaKey });

describe("primary", () => {
  it("is Ctrl off a Mac, whatever Win or Super does", () => {
    expect(primary(held(true, false), false)).toBe(true);
    expect(primary(held(false, true), false)).toBe(false);
    expect(primary(held(true, true), false)).toBe(true);
    expect(primary(held(false, false), false)).toBe(false);
  });

  it("is Cmd on a Mac, whatever Ctrl does", () => {
    expect(primary(held(false, true), true)).toBe(true);
    expect(primary(held(true, false), true)).toBe(false);
    expect(primary(held(true, true), true)).toBe(true);
    expect(primary(held(false, false), true)).toBe(false);
  });
});

describe("foreign", () => {
  it("is the key primary is not", () => {
    expect(foreign(held(false, true), false)).toBe(true);
    expect(foreign(held(true, false), false)).toBe(false);
    expect(foreign(held(true, false), true)).toBe(true);
    expect(foreign(held(false, true), true)).toBe(false);
  });
});

/** `ctrlKey || metaKey` took Win+Z for Ctrl+Z (Discard in Files) and, on a Mac, the
    system's Ctrl+click for "add to the selection". */
const SRC = join(__dirname, "..");
const EITHER = /\b(?:ctrlKey\s*\|\|\s*[\w.]*metaKey|metaKey\s*\|\|\s*[\w.]*ctrlKey)\b/;

function files(dir: string, found: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) files(path, found);
    else if (/\.(svelte|ts)$/.test(name) && !/\.(test|d)\.ts$/.test(name)) found.push(path);
  }
  return found;
}

describe("the Ctrl or Cmd of a handler", () => {
  it("is decided by primary, never by taking either key", () => {
    const either = files(SRC).filter((file) => EITHER.test(readFileSync(file, "utf8")));
    expect(either.map((file) => file.slice(SRC.length))).toEqual([]);
  });
});
