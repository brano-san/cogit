import { describe, expect, it } from "vitest";
import { NETWORK_FIELDS, changes, ticked, toWrite } from "./network-settings";

function field(key: string) {
  const found = NETWORK_FIELDS.find((entry) => entry.key === key);
  if (!found) throw new Error(`${key} is not a field`);
  return found;
}

describe("network settings", () => {
  it("offers proxy, TLS checking and the CA file, in that order", () => {
    expect(NETWORK_FIELDS.map((entry) => entry.key)).toEqual(["http.proxy", "http.sslVerify", "http.sslCAInfo"]);
    expect(field("http.sslCAInfo").control.kind).toBe("path");
  });

  it("removes a key cleared to nothing and keeps text as typed, trimmed", () => {
    expect(toWrite(field("http.proxy"), "  ")).toBeNull();
    expect(toWrite(field("http.proxy"), " http://proxy.corp:3128 ")).toBe("http://proxy.corp:3128");
    expect(toWrite(field("http.sslCAInfo"), "C:/certs/corp.pem")).toBe("C:/certs/corp.pem");
  });

  it("writes false for an unticked TLS check and removes the key for a tick, git's default", () => {
    expect(toWrite(field("http.sslVerify"), false)).toBe("false");
    expect(toWrite(field("http.sslVerify"), true)).toBeNull();
  });

  it("shows TLS checking on unless git is told otherwise", () => {
    const verify = field("http.sslVerify");
    expect(ticked(verify, null)).toBe(true);
    expect(ticked(verify, "false")).toBe(false);
    expect(ticked(verify, "no")).toBe(false);
    expect(ticked(verify, "true")).toBe(true);
  });

  it("writes only what differs from what was read", () => {
    const read = new Map<string, string | null>([["http.proxy", "http://a:1"], ["http.sslVerify", null]]);
    expect(changes(read, "http.proxy", "http://a:1")).toBe(false);
    expect(changes(read, "http.proxy", null)).toBe(true);
    expect(changes(read, "http.sslVerify", null)).toBe(false);
    expect(changes(read, "http.sslCAInfo", null)).toBe(false);
  });
});
