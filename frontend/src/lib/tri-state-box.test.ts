import { describe, expect, it } from "vitest";
import type { Branch, Tag } from "$lib/ipc";
import { buildRefTree, checkState, toggleNode, type CheckState } from "./ref-nodes";
import { faceOf, triState } from "./tri-state-box";

const OID = "a".repeat(40);

class FakeBox extends EventTarget {
  checked = false;
  indeterminate = false;
}

function branch(name: string, isHead = false): Branch {
  return {
    name,
    fullName: `refs/heads/${name}`,
    kind: "local",
    oid: OID,
    isHead,
    upstream: null,
    ahead: 0,
    behind: 0,
  };
}

function tag(name: string, pointsToCommit = true): Tag {
  return { name, fullName: `refs/tags/${name}`, oid: OID, isAnnotated: true, pointsToCommit };
}

/** A user's click in the order the HTML spec runs it: the tick flips, the listeners run,
    the microtask checkpoint after a listener lets Svelte render, and only then does a
    cancelled click put the old tick back; a click left alone fires `change`. */
function userClick(box: FakeBox, render: () => void): void {
  const before = { checked: box.checked, indeterminate: box.indeterminate };
  box.checked = !box.checked;
  box.indeterminate = false;
  const click = new Event("click", { bubbles: true, cancelable: true });
  box.dispatchEvent(click);
  render();
  if (click.defaultPrevented) {
    box.checked = before.checked;
    box.indeterminate = before.indeterminate;
    return;
  }
  box.dispatchEvent(new Event("change", { bubbles: true }));
  render();
}

/** The Branches panel as Svelte drives it: an action per row, updated when its state changes. */
function panel(start: (ids: string[]) => Set<string>) {
  const tree = buildRefTree({
    head: { kind: "branch", name: "master", oid: OID },
    branches: [branch("master", true), branch("dev"), branch("feature/x")],
    tags: [tag("v1"), tag("v2"), tag("tree-tag", false)],
    stashes: [],
    lost: [],
    remoteUrls: {},
    collapsed: new Set(),
    filter: "",
  });
  let visible = start(tree.map((node) => node.id));
  const boxes = new Map<string, FakeBox>();
  const rendered = new Map<string, CheckState>();
  const actions = new Map<string, ReturnType<typeof triState>>();

  const params = (id: string) => ({
    state: checkState(tree, id, visible),
    toggle: () => {
      visible = toggleNode(tree, id, visible);
      return checkState(tree, id, visible);
    },
  });

  for (const node of tree) {
    const box = new FakeBox();
    boxes.set(node.id, box);
    rendered.set(node.id, checkState(tree, node.id, visible));
    actions.set(node.id, triState(box, params(node.id)));
  }

  const render = () => {
    for (const node of tree) {
      const state = checkState(tree, node.id, visible);
      if (rendered.get(node.id) === state) continue;
      rendered.set(node.id, state);
      actions.get(node.id)!.update(params(node.id));
    }
  };

  return {
    click: (id: string) => userClick(boxes.get(id)!, render),
    state: (id: string) => checkState(tree, id, visible),
    face: (id: string) => faceOf(boxes.get(id)!),
    ids: tree.map((node) => node.id),
  };
}

function expectShown(view: ReturnType<typeof panel>, expected: Record<string, CheckState>) {
  for (const [id, state] of Object.entries(expected)) {
    expect(view.state(id), `${id} state`).toBe(state);
    expect(view.face(id), `${id} box`).toBe(state);
  }
  for (const id of view.ids) expect(view.face(id), `${id} box`).toBe(view.state(id));
}

describe("the Branches check boxes, clicked the way a user clicks them", () => {
  it("run the whole cycle on a group and on its children", () => {
    const view = panel((ids) => new Set(ids.filter((id) => id.startsWith("local:"))));
    expectShown(view, { "group:local": "on" });

    view.click("group:local");
    expectShown(view, { "group:local": "off", "local:master": "off", "local:dev": "off" });

    view.click("local:dev");
    expectShown(view, { "local:dev": "on", "group:local": "mixed" });

    view.click("group:local");
    expectShown(view, { "group:local": "on", "local:master": "on", "local:feature/x": "on" });

    view.click("group:local");
    expectShown(view, { "group:local": "off", "local:dev": "off", "folder:local/feature": "off" });

    view.click("local:master");
    expectShown(view, { "local:master": "on", "group:local": "mixed" });

    view.click("local:master");
    expectShown(view, { "local:master": "off", "group:local": "off" });
  });

  it("empty a group whose inactive child the mass change cannot touch", () => {
    const view = panel(() => new Set());
    view.click("group:tags");
    expectShown(view, { "group:tags": "on", "tag:v1": "on", "tag:tree-tag": "off" });

    view.click("group:tags");
    expectShown(view, { "group:tags": "off", "tag:v1": "off" });

    view.click("tag:v2");
    view.click("group:tags");
    expectShown(view, { "group:tags": "on" });
    view.click("group:tags");
    expectShown(view, { "group:tags": "off" });
  });

  it("let a folder inside a group tick and untick like any other heading", () => {
    const view = panel(() => new Set());
    view.click("folder:local/feature");
    expectShown(view, { "folder:local/feature": "on", "group:local": "mixed" });
    view.click("folder:local/feature");
    expectShown(view, { "folder:local/feature": "off", "group:local": "off" });
  });
});
