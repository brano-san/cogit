import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { ACTIONS, commandOf, perspectiveOf } from "./menu";

/** The menu itself is built in Rust; the page must understand every item it can send. */
function rustActions(): string[] {
  const source = readFileSync(
    fileURLToPath(new URL("../../../../src-tauri/src/commands/investigate.rs", import.meta.url)),
    "utf8",
  );
  return [...source.matchAll(/item\(\s*"([a-z-]+)"/g)].map((match) => match[1]!);
}

describe("Investigate menu actions", () => {
  it("covers every item of the native menu but Close, which Rust answers", () => {
    expect([...ACTIONS].sort()).toEqual(rustActions().sort());
  });

  it("turns an action into a command and refuses anything else", () => {
    expect(commandOf("go-deeper")).toBe("go-deeper");
    expect(commandOf("close")).toBeNull();
    expect(commandOf("rm -rf")).toBeNull();
  });

  it("maps the Window menu to the five perspectives", () => {
    expect(perspectiveOf("perspective-blame-origins")).toBe("blameOrigins");
    expect(perspectiveOf("perspective-log")).toBe("log");
    expect(perspectiveOf("back")).toBeNull();
  });
});
