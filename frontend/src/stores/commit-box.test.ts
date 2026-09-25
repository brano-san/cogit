import { describe, expect, it, vi } from "vitest";

const { commitBox } = await import("./commit-box.svelte");
const { layout } = await import("./layout.svelte");

function field() {
  return { focus: vi.fn(), submit: vi.fn(async (_amend: boolean) => {}) };
}

// Local ▸ Commit…, its Ctrl+Enter and the palette's Commit Staged ran `() => {}`; with the
// Commit Message panel hidden the Working Tree row's Commit had no field to focus.
describe("commitBox", () => {
  it("commits from the menu and the palette", async () => {
    const box = field();
    const detach = commitBox.attach(box);
    await commitBox.commit();
    expect(box.focus).toHaveBeenCalled();
    expect(box.submit).toHaveBeenCalledWith(false);
    detach();
  });

  it("amends for Ctrl+Shift+Enter", async () => {
    const box = field();
    const detach = commitBox.attach(box);
    await commitBox.commit(true);
    expect(box.submit).toHaveBeenCalledWith(true);
    detach();
  });

  it("only puts the cursor in the field for Ctrl+K", async () => {
    const box = field();
    const detach = commitBox.attach(box);
    await commitBox.focus();
    expect(box.focus).toHaveBeenCalled();
    expect(box.submit).not.toHaveBeenCalled();
    detach();
  });

  it("shows the Commit Message panel first when it is hidden", async () => {
    if (layout.visible("commit")) layout.togglePanel("commit");
    await commitBox.focus();
    expect(layout.visible("commit")).toBe(true);
  });

  it("does nothing through a box that has gone", async () => {
    const box = field();
    commitBox.attach(box)();
    await commitBox.commit();
    expect(box.submit).not.toHaveBeenCalled();
  });
});
