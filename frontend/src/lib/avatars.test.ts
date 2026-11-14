import { describe, expect, it } from "vitest";
import { authorsOf, initialsOf, mergeRows } from "./avatars";
import type { AvatarRow } from "$lib/ipc";

const row = (email: string, image: string | null = null): AvatarRow => ({
  email,
  initials: "XX",
  color: "#123456",
  image,
});

describe("authorsOf", () => {
  it("keeps one entry per address", () => {
    const authors = authorsOf([
      { authorName: "Ada", authorEmail: "ada@example.com" },
      { authorName: "Ada Lovelace", authorEmail: "ada@example.com" },
      { authorName: "Grace", authorEmail: "grace@example.com" },
    ]);
    expect(authors).toEqual([
      { name: "Ada", email: "ada@example.com" },
      { name: "Grace", email: "grace@example.com" },
    ]);
  });

  it("keeps the order of the window so the top rows are fetched first", () => {
    const authors = authorsOf([
      { authorName: "B", authorEmail: "b@example.com" },
      { authorName: "A", authorEmail: "a@example.com" },
    ]);
    expect(authors.map((a) => a.email)).toEqual(["b@example.com", "a@example.com"]);
  });

  it("treats one address in two spellings as one author", () => {
    const authors = authorsOf([
      { authorName: "Ada", authorEmail: "Ada@Example.com" },
      { authorName: "Ada", authorEmail: "ada@example.com " },
    ]);
    expect(authors).toHaveLength(1);
  });

  it("drops an author with no address rather than asking for nothing", () => {
    expect(authorsOf([{ authorName: "Nobody", authorEmail: "  " }])).toEqual([]);
  });
});

describe("mergeRows", () => {
  /** A fresh table with `batches` merged into it in turn. */
  const merged = (...batches: AvatarRow[][]) => {
    const rows = new Map<string, AvatarRow>();
    for (const batch of batches) mergeRows(rows, batch);
    return rows;
  };

  it("adds what arrived", () => {
    expect(merged([row("ada@example.com")]).get("ada@example.com")?.initials).toBe("XX");
  });

  it("replaces a fallback row once the picture lands", () => {
    const rows = merged([row("ada@example.com")], [row("ada@example.com", "data:image/png;base64,AA")]);
    expect(rows.get("ada@example.com")?.image).toBe("data:image/png;base64,AA");
  });

  it("does not lose a picture to a later row that has none", () => {
    // A second window can answer before the queue has finished; forgetting the picture
    // would make the row flicker back to initials.
    const rows = merged([row("ada@example.com", "data:image/png;base64,AA")], [row("ada@example.com")]);
    expect(rows.get("ada@example.com")?.image).toBe("data:image/png;base64,AA");
  });

  it("leaves rows outside the window alone", () => {
    expect(merged([row("grace@example.com")], [row("ada@example.com")]).has("grace@example.com")).toBe(true);
  });

  it("keys on the normalised address", () => {
    expect(merged([row(" Ada@Example.com ")]).has("ada@example.com")).toBe(true);
  });

  it("merges into the table it is given, a reactive one too", () => {
    const rows = new Map<string, AvatarRow>();
    mergeRows(rows, [row("ada@example.com")]);
    expect(rows.size).toBe(1);
  });
});

/** The backend draws the same letters (crates/avatars/tests/identity.rs); a row that
    changes its initials when the answer arrives is worse than waiting for it. */
describe("initialsOf", () => {
  it("takes the first letter of the first and last word", () => {
    expect(initialsOf("Ada Lovelace", "ada@example.com")).toBe("AL");
    expect(initialsOf("Ada Augusta King Lovelace", "ada@example.com")).toBe("AL");
  });

  it("gives one letter for a single-word name", () => {
    expect(initialsOf("octocat", "o@example.com")).toBe("O");
  });

  it("keeps the letters of a non-latin name", () => {
    expect(initialsOf("Иван Петров", "ivan@example.com")).toBe("ИП");
  });

  it("falls back to the address when there is no name", () => {
    expect(initialsOf("   ", "ada@example.com")).toBe("A");
  });

  it("always has something to draw", () => {
    expect(initialsOf("", "")).toBe("?");
  });
});
