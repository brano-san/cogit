import { beforeEach, describe, expect, it, vi } from "vitest";

const ipc = vi.hoisted(() => ({
  readEditable: vi.fn(),
  saveEditable: vi.fn(),
}));
const ask = vi.hoisted(() => vi.fn());

vi.mock("$lib/ipc/editable", () => ipc);
vi.mock("$stores/unsaved-prompt.svelte", () => ({ unsavedPrompt: { ask } }));

const { diffEdit } = await import("./diff-edit.svelte");

const shape = { encoding: "utf8", bom: false, eol: "crlf", finalNewline: true } as const;
const file = (text: string, stamp: string) => ({ kind: "text", base: "old", text, shape, stamp });
const spec = { kind: "workTreeVsIndex" } as const;
const repo = 1 as never;

beforeEach(async () => {
  vi.clearAllMocks();
  diffEdit.stop();
  ipc.readEditable.mockResolvedValue(file("a", "s1"));
  await diffEdit.start(repo, spec, "a.txt");
  diffEdit.attach({ text: () => "edited", focused: () => true, save: () => {} });
});

describe("diffEdit", () => {
  it("saves with the shape and stamp it read, and takes the new stamp", async () => {
    ipc.saveEditable.mockResolvedValue({ kind: "saved", stamp: "s2" });
    diffEdit.dirty = true;

    expect(await diffEdit.save("edited")).toBe(true);

    expect(ipc.saveEditable).toHaveBeenCalledWith(repo, "a.txt", "edited", shape, "s1", false);
    expect(diffEdit.opened?.stamp).toBe("s2");
    expect(diffEdit.dirty).toBe(false);
  });

  it("asks Reload or Keep Mine when the file moved on, and writes nothing", async () => {
    ipc.saveEditable.mockResolvedValue({ kind: "changedOnDisk" });
    expect(await diffEdit.save("edited")).toBe(false);
    expect(diffEdit.changedOnDisk).toBe(true);
  });

  it("follows the disk while clean, asks while dirty, ignores its own save", async () => {
    ipc.readEditable.mockResolvedValue(file("a", "s1"));
    await diffEdit.diskChanged();
    expect(diffEdit.generation).toBe(0);

    ipc.readEditable.mockResolvedValue(file("theirs", "s9"));
    await diffEdit.diskChanged();
    expect(diffEdit.opened?.text).toBe("theirs");
    expect(diffEdit.generation).toBe(1);

    diffEdit.dirty = true;
    ipc.readEditable.mockResolvedValue(file("again", "s10"));
    await diffEdit.diskChanged();
    expect(diffEdit.changedOnDisk).toBe(true);
    expect(diffEdit.opened?.text).toBe("theirs");
  });

  it("never leaves unsaved edits behind without asking", async () => {
    diffEdit.dirty = true;
    ask.mockResolvedValue("save");
    ipc.saveEditable.mockResolvedValue({ kind: "saved", stamp: "s2" });

    expect(await diffEdit.leave()).toBe(true);

    expect(ask).toHaveBeenCalledOnce();
    expect(ipc.saveEditable).toHaveBeenCalledWith(repo, "a.txt", "edited", shape, "s1", false);
    expect(diffEdit.active).toBe(false);
  });

  it("stays on the file when the question is answered Cancel", async () => {
    diffEdit.dirty = true;
    ask.mockResolvedValue("cancel");

    expect(await diffEdit.leaveFor("other.txt", spec)).toBe(false);

    expect(ipc.saveEditable).not.toHaveBeenCalled();
    expect(diffEdit.opened?.path).toBe("a.txt");
  });

  it("lets the same file through without a question", async () => {
    diffEdit.dirty = true;
    expect(await diffEdit.leaveFor("a.txt", spec)).toBe(true);
    expect(ask).not.toHaveBeenCalled();
  });
});
