import { describe, expect, it, vi } from "vitest";
import { suppressNativeMenu } from "./native-menu";

/** A document stand-in: records the listener and lets a test fire events at it. */
function fakeDocument(selection = "") {
  const listeners: ((event: MouseEvent) => void)[] = [];
  const doc = {
    addEventListener: (_: string, fn: (event: MouseEvent) => void) => listeners.push(fn),
    removeEventListener: vi.fn(),
    getSelection: () => ({ isCollapsed: selection === "", toString: () => selection }),
  } as unknown as Document;

  const fire = (over: Partial<MouseEvent> & { target?: unknown }) => {
    const event = {
      defaultPrevented: false,
      preventDefault() {
        (this as { defaultPrevented: boolean }).defaultPrevented = true;
      },
      target: null,
      ...over,
    } as unknown as MouseEvent;
    for (const fn of listeners) fn(event);
    return event;
  };

  return { doc, fire };
}

const element = (matches: string | null, doc: Document) =>
  ({
    ownerDocument: doc,
    closest: (selector: string) => (selector === matches ? {} : null),
  }) as unknown as Element;

describe("suppressNativeMenu", () => {
  it("stops the webview's own menu where nothing else claimed the click", () => {
    const { doc, fire } = fakeDocument();
    suppressNativeMenu(doc);

    expect(fire({}).defaultPrevented).toBe(true);
  });

  it("leaves a panel that opened its own menu alone", () => {
    const { doc, fire } = fakeDocument();
    suppressNativeMenu(doc);

    const event = fire({ defaultPrevented: true });
    expect(event.defaultPrevented).toBe(true);
  });

  // Copying selected output is worth the platform's menu; a Git client's controls are not.
  it("keeps the platform menu over selected text", () => {
    const { doc, fire } = fakeDocument("error: failed to push");
    suppressNativeMenu(doc);

    expect(fire({ target: element(null, doc) }).defaultPrevented).toBe(false);
  });

  it("keeps it over a text field, where cut and paste live", () => {
    const { doc, fire } = fakeDocument();
    suppressNativeMenu(doc);

    expect(fire({ target: element("input, textarea", doc) }).defaultPrevented).toBe(false);
  });

  it("can be taken off again", () => {
    const { doc } = fakeDocument();
    const stop = suppressNativeMenu(doc);
    stop();
    expect(doc.removeEventListener).toHaveBeenCalled();
  });
});
