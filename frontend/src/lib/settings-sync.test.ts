import { describe, expect, it, vi } from "vitest";
import { SETTINGS_CHANGED, announceSettings, followSettings, type Listen } from "./settings-sync";

type Handler = (event: { payload: unknown }) => void;

function bus() {
  const handlers: Handler[] = [];
  const listen: Listen = async (name, handler) => {
    expect(name).toBe(SETTINGS_CHANGED);
    handlers.push(handler as Handler);
    return () => void handlers.splice(handlers.indexOf(handler as Handler), 1);
  };
  const emit = async (name: string, payload: unknown) => {
    expect(name).toBe(SETTINGS_CHANGED);
    for (const handler of [...handlers]) handler({ payload });
  };
  return { listen, emit, handlers };
}

// Preferences ▸ Theme recoloured the main window only: an open Blame stayed dark until it
// was opened again (F-335).
describe("followSettings", () => {
  it("reads the settings again when another window wrote them", async () => {
    const { listen, emit } = bus();
    const reread = vi.fn();
    followSettings(reread, listen);
    await Promise.resolve();
    await emit(SETTINGS_CHANGED, "another-window");
    expect(reread).toHaveBeenCalledTimes(1);
  });

  it("does not read again what this window wrote itself", async () => {
    const { listen, emit } = bus();
    const reread = vi.fn();
    followSettings(reread, listen);
    await Promise.resolve();
    announceSettings(emit);
    expect(reread).not.toHaveBeenCalled();
  });

  it("stops listening when undone, even before the listener was in place", async () => {
    const { listen, emit, handlers } = bus();
    const reread = vi.fn();
    followSettings(reread, listen)();
    await Promise.resolve();
    await Promise.resolve();
    await emit(SETTINGS_CHANGED, "another-window");
    expect(reread).not.toHaveBeenCalled();
    expect(handlers).toHaveLength(0);
  });
});
