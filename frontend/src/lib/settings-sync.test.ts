import { describe, expect, it, vi } from "vitest";
import { announceSettings, followSettings, type Emit, type Listen } from "./settings-sync";

type Handler = (event: { payload: string }) => void;

function bus() {
  const handlers: Handler[] = [];
  const listen: Listen<string> = async (handler) => {
    handlers.push(handler);
    return () => void handlers.splice(handlers.indexOf(handler), 1);
  };
  const emit: Emit<string> = async (payload) => {
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
    await emit("another-window");
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
    await emit("another-window");
    expect(reread).not.toHaveBeenCalled();
    expect(handlers).toHaveLength(0);
  });
});
