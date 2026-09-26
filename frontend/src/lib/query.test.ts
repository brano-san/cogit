import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { textFields } from "./filter-fields";
import { formatQuery, isEmptyQuery, parseQuery, sameQuery } from "./query";

describe("parseQuery", () => {
  it("looks for bare words in the fields the switches pick", () => {
    expect(parseQuery("fix the parser")).toMatchObject({
      text: "fix the parser",
      textIn: textFields(["author", "committer", "message", "refs", "id"]),
      message: null,
    });
    expect(parseQuery("fix", ["content"]).textIn).toEqual(textFields(["content"]));
  });

  it("returns an empty query for blank input", () => {
    const query = parseQuery("   ");
    expect(isEmptyQuery(query)).toBe(true);
    expect(query.text).toBeNull();
    expect(query.textIn).toBeUndefined();
  });

  it("reads a prefixed field", () => {
    expect(parseQuery("author:brano")).toMatchObject({ author: "brano", text: null });
  });

  it("combines fields with free text", () => {
    expect(parseQuery("author:brano parser path:src/lib")).toMatchObject({
      author: "brano",
      path: "src/lib",
      text: "parser",
    });
  });

  it("accepts a quoted value with spaces", () => {
    expect(parseQuery('author:"John Doe" fix')).toMatchObject({
      author: "John Doe",
      text: "fix",
    });
  });

  it("parses a date into unix seconds at the start of that day, in the user's time zone", () => {
    expect(parseQuery("since:2026-01-01").since).toBe(new Date(2026, 0, 1).getTime() / 1000);
  });

  it("makes until inclusive to the end of that day", () => {
    const query = parseQuery("until:2026-01-01");
    expect(query.until).toBe(new Date(2026, 0, 2).getTime() / 1000 - 1);
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

  it("keeps an unknown prefix as part of the text", () => {
    expect(parseQuery("ticket:123").text).toBe("ticket:123");
  });

  it("does not treat a bare colon as a field", () => {
    expect(parseQuery("fix: crash").text).toBe("fix: crash");
  });
});

// The graph shows a commit's date in its own zone; a UTC midnight hid the first hours of
// the user's day from since: and added them to until:.
describe("dates east of Greenwich", () => {
  const zone = process.env.TZ;
  beforeAll(() => {
    process.env.TZ = "Europe/Moscow";
  });
  afterAll(() => {
    if (zone === undefined) delete process.env.TZ;
    else process.env.TZ = zone;
  });

  // 2026-01-01 01:30 in Moscow, 2025-12-31 22:30 in UTC.
  const commit = Date.UTC(2025, 11, 31, 22, 30) / 1000;

  it("keep a commit made in the first hours of the day since it", () => {
    expect(parseQuery("since:2026-01-01").since).toBeLessThanOrEqual(commit);
    expect(parseQuery("until:2025-12-31").until).toBeLessThan(commit);
  });

  it("read back as the days typed", () => {
    const text = "since:2026-01-01 until:2026-01-31";
    expect(formatQuery(parseQuery(text))).toBe(text);
  });
});

describe("isEmptyQuery", () => {
  it("is true only when nothing is set", () => {
    expect(isEmptyQuery(parseQuery(""))).toBe(true);
    expect(isEmptyQuery(parseQuery("path:src"))).toBe(false);
    expect(isEmptyQuery(parseQuery("fix"))).toBe(false);
  });

  it("does not count the switches without text", () => {
    expect(isEmptyQuery({ ...parseQuery(""), textIn: textFields(["content"]) })).toBe(true);
  });
});

describe("sameQuery", () => {
  it("tells text looked for in other fields apart", () => {
    expect(sameQuery(parseQuery("fix", ["message"]), parseQuery("fix", ["message", "name"]))).toBe(false);
    expect(sameQuery(parseQuery("fix", ["message"]), parseQuery("fix", ["message"]))).toBe(true);
  });

  it("ignores the switches while there is no text", () => {
    expect(sameQuery(parseQuery("path:src", ["message"]), parseQuery("path:src", ["name"]))).toBe(true);
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
