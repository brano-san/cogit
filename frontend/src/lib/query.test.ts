import { describe, expect, it } from "vitest";
import { formatQuery, isEmptyQuery, parseQuery } from "./query";

describe("parseQuery", () => {
  it("treats bare words as a message filter", () => {
    expect(parseQuery("fix the parser")).toMatchObject({ message: "fix the parser" });
  });

  it("returns an empty query for blank input", () => {
    const query = parseQuery("   ");
    expect(isEmptyQuery(query)).toBe(true);
    expect(query.message).toBeNull();
  });

  it("reads a prefixed field", () => {
    expect(parseQuery("author:brano")).toMatchObject({ author: "brano", message: null });
  });

  it("combines fields with free text", () => {
    expect(parseQuery("author:brano parser path:src/lib")).toMatchObject({
      author: "brano",
      path: "src/lib",
      message: "parser",
    });
  });

  it("accepts a quoted value with spaces", () => {
    expect(parseQuery('author:"John Doe" fix')).toMatchObject({
      author: "John Doe",
      message: "fix",
    });
  });

  it("parses a date into unix seconds at the start of that day, in UTC", () => {
    expect(parseQuery("since:2026-01-01").since).toBe(Date.UTC(2026, 0, 1) / 1000);
  });

  it("makes until inclusive to the end of that day", () => {
    const query = parseQuery("until:2026-01-01");
    expect(query.until).toBe(Date.UTC(2026, 0, 2) / 1000 - 1);
  });

  it("ignores a date it cannot parse rather than filtering everything out", () => {
    expect(parseQuery("since:yesterday").since).toBeNull();
  });

  it("accepts oid as a prefix filter", () => {
    expect(parseQuery("oid:4f3a2b1").oidPrefix).toBe("4f3a2b1");
  });

  it("lets the last occurrence of a field win", () => {
    expect(parseQuery("author:a author:b").author).toBe("b");
  });

  it("keeps an unknown prefix as part of the message", () => {
    expect(parseQuery("ticket:123").message).toBe("ticket:123");
  });

  it("does not treat a bare colon as a field", () => {
    expect(parseQuery("fix: crash").message).toBe("fix: crash");
  });
});

describe("isEmptyQuery", () => {
  it("is true only when nothing is set", () => {
    expect(isEmptyQuery(parseQuery(""))).toBe(true);
    expect(isEmptyQuery(parseQuery("path:src"))).toBe(false);
  });
});

// File ▸ Log filtered the graph by path while the field stayed empty, with no count and no
// ✕: the filter could not be seen, nor cleared from where filters are typed.
describe("formatQuery", () => {
  it("writes a query as the field would have been typed", () => {
    expect(formatQuery(parseQuery("path:src/lib"))).toBe("path:src/lib");
    expect(formatQuery(parseQuery(""))).toBe("");
  });

  it("reads back as the same query", () => {
    for (const text of [
      'author:"Brano San" path:src since:2026-01-01 until:2026-02-01 fix the graph',
      "oid:abc123",
      'path:"docs/with space/a.md"',
    ]) {
      const query = parseQuery(text);
      expect(parseQuery(formatQuery(query))).toEqual(query);
    }
  });
});
