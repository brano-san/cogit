// A minimal DevTools Protocol client: the same shape as scripts/oom/probe.mjs, plus input.

export const sleep = (ms) => new Promise((ok) => setTimeout(ok, ms));

/** The window takes a moment to exist; a refused connection here is normal, not fatal. */
export async function pageTarget(port, timeoutMs = 60_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json`);
      const targets = await response.json();
      const page = targets.find((t) => t.type === "page" && t.webSocketDebuggerUrl && !t.url.startsWith("devtools"));
      if (page) return page;
    } catch {
      // Not listening yet.
    }
    await sleep(50);
  }
  throw new Error(`no page target on port ${port} after ${timeoutMs}ms`);
}

export async function connect(url) {
  const socket = new WebSocket(url);
  const pending = new Map();
  const listeners = new Map();
  let nextId = 1;

  await new Promise((ok, fail) => {
    socket.addEventListener("open", ok, { once: true });
    socket.addEventListener("error", () => fail(new Error("cdp socket failed")), { once: true });
  });

  socket.addEventListener("message", (event) => {
    const frame = JSON.parse(event.data);
    if (frame.id !== undefined) {
      const slot = pending.get(frame.id);
      if (!slot) return;
      pending.delete(frame.id);
      frame.error ? slot.fail(new Error(JSON.stringify(frame.error))) : slot.ok(frame.result);
      return;
    }
    for (const handler of listeners.get(frame.method) ?? []) handler(frame.params);
  });
  socket.addEventListener("close", () => {
    for (const slot of pending.values()) slot.fail(new Error("cdp socket closed"));
    pending.clear();
  });

  const cdp = {
    send(method, params = {}) {
      const id = nextId++;
      socket.send(JSON.stringify({ id, method, params }));
      return new Promise((ok, fail) => pending.set(id, { ok, fail }));
    },
    on(method, handler) {
      listeners.set(method, [...(listeners.get(method) ?? []), handler]);
    },
    close: () => socket.close(),

    /** A page that navigated away can leave an evaluation unanswered; nothing waits forever. */
    async eval(expression, timeoutMs = 240_000) {
      let timer;
      const result = await Promise.race([
        cdp.send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true }),
        new Promise((_, fail) => {
          timer = setTimeout(() => fail(new Error(`page did not answer in ${timeoutMs / 1000}s`)), timeoutMs);
        }),
      ]).finally(() => clearTimeout(timer));
      if (result.exceptionDetails) {
        const detail = result.exceptionDetails.exception?.description ?? result.exceptionDetails.text;
        throw new Error(`page: ${detail}\n  in: ${expression.slice(0, 160)}`);
      }
      return result.result.value;
    },

    async click(x, y, { button = "left", clickCount = 1 } = {}) {
      const base = { x, y, button, clickCount };
      await cdp.send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
      await cdp.send("Input.dispatchMouseEvent", { type: "mousePressed", ...base });
      await cdp.send("Input.dispatchMouseEvent", { type: "mouseReleased", ...base });
    },

    async doubleClick(x, y) {
      await cdp.click(x, y, { clickCount: 1 });
      await cdp.click(x, y, { clickCount: 2 });
    },

    /** Off every element with a tooltip, so a hover timer does not fire mid-measurement. */
    async park() {
      await cdp.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: 2, y: 2 });
    },

    async key(key, { code = key, keyCode = 0, modifiers = 0, text } = {}) {
      const base = { key, code, windowsVirtualKeyCode: keyCode, nativeVirtualKeyCode: keyCode, modifiers };
      await cdp.send("Input.dispatchKeyEvent", { type: text ? "keyDown" : "rawKeyDown", text, ...base });
      await cdp.send("Input.dispatchKeyEvent", { type: "keyUp", ...base });
    },

    async type(text) {
      await cdp.send("Input.insertText", { text });
    },
  };
  return cdp;
}

export const KEYS = {
  ArrowDown: { key: "ArrowDown", code: "ArrowDown", keyCode: 40 },
  Enter: { key: "Enter", code: "Enter", keyCode: 13, text: "\r" },
  Escape: { key: "Escape", code: "Escape", keyCode: 27 },
  Space: { key: " ", code: "Space", keyCode: 32, text: " " },
};
