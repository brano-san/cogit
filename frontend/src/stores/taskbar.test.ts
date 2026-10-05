import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {} }));
vi.mock("$lib/ipc/bindings", () => ({ commands: {}, events: {} }));

const { taskbar } = await import("./taskbar.svelte");

describe("taskbar events", () => {
  it("counts nothing while the window has focus", () => {
    taskbar.setFocused(true);
    taskbar.event("error");
    expect(taskbar.unviewed).toBe(0);
  });

  it("counts each event in the background and forgets them on focus", () => {
    taskbar.setFocused(false);
    taskbar.event("success");
    taskbar.event("warning");
    expect(taskbar.unviewed).toBe(2);
    taskbar.setFocused(true);
    expect(taskbar.unviewed).toBe(0);
  });

  it("flashes for an error, never for a success", () => {
    taskbar.setFocused(false);
    taskbar.event("success");
    expect(taskbar.flash).toBeNull();
    taskbar.event("error");
    expect(taskbar.flash).toBe("persistent");
    taskbar.setFocused(true);
    expect(taskbar.flash).toBeNull();
  });
});
