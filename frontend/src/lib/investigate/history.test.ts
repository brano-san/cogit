import { describe, expect, it } from "vitest";
import {
  HISTORY_LIMIT,
  backList,
  canGoBack,
  canGoForward,
  current,
  goBack,
  goForward,
  goTo,
  locationLabel,
  pushLocation,
  startHistory,
} from "./history";

const at = (rev: string | null, line: number | null = null, path = "src/a.rs") => ({
  path,
  rev,
  line,
});

describe("investigation history", () => {
  it("starts with one place and nowhere to go", () => {
    const history = startHistory(at("c1"));
    expect(canGoBack(history)).toBe(false);
    expect(canGoForward(history)).toBe(false);
  });

  it("goes back and forward over the steps taken", () => {
    let history = pushLocation(pushLocation(startHistory(at("c1")), at("c2")), at("c3"));
    history = goBack(goBack(history));
    expect(current(history)).toEqual(at("c1"));
    expect(canGoForward(history)).toBe(true);
    history = goForward(history);
    expect(current(history)).toEqual(at("c2"));
  });

  it("a new step after going back drops what Forward held", () => {
    let history = pushLocation(pushLocation(startHistory(at("c1")), at("c2")), at("c3"));
    history = pushLocation(goBack(goBack(history)), at("c9"));
    expect(history.entries.map((entry) => entry.rev)).toEqual(["c1", "c9"]);
    expect(canGoForward(history)).toBe(false);
  });

  it("another line of the same version replaces the step instead of adding one", () => {
    const history = pushLocation(startHistory(at("c1", 3)), at("c1", 8));
    expect(history.entries).toEqual([at("c1", 8)]);
  });

  it("another file at the same version is a step of its own", () => {
    const history = pushLocation(startHistory(at("c1", 3)), at("c1", 3, "src/b.rs"));
    expect(history.entries).toHaveLength(2);
  });

  it("keeps a bounded number of steps", () => {
    let history = startHistory(at("c0"));
    for (let n = 1; n <= HISTORY_LIMIT + 20; n++) history = pushLocation(history, at(`c${n}`));
    expect(history.entries).toHaveLength(HISTORY_LIMIT);
    expect(current(history).rev).toBe(`c${HISTORY_LIMIT + 20}`);
  });

  it("lists the earlier places most recent first, for the Back dropdown", () => {
    const history = pushLocation(pushLocation(startHistory(at("c1")), at("c2")), at("c3"));
    expect(backList(history).map((item) => item.location.rev)).toEqual(["c2", "c1"]);
    expect(current(goTo(history, backList(history)[1]!.index)).rev).toBe("c1");
  });

  it("labels a place by file, short hash or working tree, and line", () => {
    expect(locationLabel(at("0123456789abcdef", 12))).toBe("a.rs @ 0123456 · line 12");
    expect(locationLabel(at(null))).toBe("a.rs @ Working Tree");
  });
});
