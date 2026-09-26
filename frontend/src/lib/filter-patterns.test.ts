import { describe, expect, it } from "vitest";
import { MAX_PATTERNS, forget, forgetBlocked, knownPatterns, remember, rememberBlocked } from "./filter-patterns";

describe("remembered filter patterns", () => {
  it("go on top, trimmed", () => {
    expect(remember(["older"], "  author:brano fix ")).toEqual(["author:brano fix", "older"]);
  });

  it("move up when remembered again instead of showing twice", () => {
    expect(remember(["a", "b", "c"], "c")).toEqual(["c", "a", "b"]);
  });

  it("keep no more than the cap, dropping the oldest", () => {
    const full = Array.from({ length: MAX_PATTERNS }, (_, i) => `p${i}`);
    const next = remember(full, "new");
    expect(next).toHaveLength(MAX_PATTERNS);
    expect(next[0]).toBe("new");
    expect(next).not.toContain(`p${MAX_PATTERNS - 1}`);
  });

  it("ignore an empty text", () => {
    expect(remember(["a"], "   ")).toEqual(["a"]);
  });

  it("forget the one named and nothing else", () => {
    expect(forget(["a", "b"], " a ")).toEqual(["b"]);
    expect(forget(["a"], "z")).toEqual(["a"]);
  });

  it("say why Remember Pattern is off", () => {
    expect(rememberBlocked([], " ")).toBe("Type a filter first");
    expect(rememberBlocked(["fix"], "fix")).toBe("Already remembered");
    expect(rememberBlocked(["fix"], "feat")).toBeNull();
  });

  it("offer Forget Pattern only for a remembered text", () => {
    expect(forgetBlocked(["fix"], "fix ")).toBeNull();
    expect(forgetBlocked(["fix"], "feat")).not.toBeNull();
  });

  it("read back from a settings file as text, once each", () => {
    expect(knownPatterns(["a", 3, " a ", "", "b"])).toEqual(["a", "b"]);
    expect(knownPatterns("a")).toBeUndefined();
  });
});
