import { describe, expect, it } from "vitest";
import { DEFAULT_FILTER_FIELDS, FILTER_FIELDS, isSlowField, knownFields, textFields, toggled } from "./filter-fields";

describe("filter fields", () => {
  it("switch on all but Name and Content by default", () => {
    expect(DEFAULT_FILTER_FIELDS).toEqual(["author", "committer", "message", "refs", "id", "notes"]);
  });

  it("show in the order of the switches row", () => {
    expect(FILTER_FIELDS).toEqual(["author", "committer", "message", "refs", "id", "name", "content", "notes"]);
  });

  it("become the fields Rust searches, every one said", () => {
    expect(textFields(["message", "content"])).toEqual({
      author: false,
      committer: false,
      message: true,
      refs: false,
      id: false,
      name: false,
      content: true,
      notes: false,
    });
    expect(textFields(["notes"]).notes).toBe(true);
  });

  it("toggle one field and keep the order", () => {
    expect(toggled(DEFAULT_FILTER_FIELDS, "committer")).toEqual(["author", "message", "refs", "id", "notes"]);
    expect(toggled(["id"], "author")).toEqual(["author", "id"]);
  });

  it("keep known fields from a settings file, once each", () => {
    expect(knownFields(["content", "bogus", "author", "author"])).toEqual(["author", "content"]);
    expect(knownFields("author")).toBeUndefined();
    expect(knownFields([])).toEqual([]);
  });

  it("mark the fields that diff trees or read blobs as slow", () => {
    expect(FILTER_FIELDS.filter(isSlowField)).toEqual(["name", "content"]);
  });
});
