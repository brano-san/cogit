// Injected into the webview; the definition of "done" is in doc/15-benchmark.md, §2.

export const PAGE = String.raw`(() => {
  if (window.__bench) return;
  const state = {
    inflight: 0,
    calls: [],
    lastActivity: 0,
    dirty: false,
    armed: null,
    input: null,
    posted: [],
  };
  const now = () => performance.now();
  const touch = () => {
    state.lastActivity = now();
    state.dirty = true;
  };

  // Tauri freezes \`__TAURI_INTERNALS__.invoke\`; on Windows every call leaves through
  // fetch to http://ipc.localhost/<command>, and fetch is ours to wrap.
  const IPC = "http://ipc.localhost/";
  const nativeFetch = window.fetch.bind(window);
  window.fetch = (input, init) => {
    const url = typeof input === "string" ? input : input?.url ?? "";
    if (!url.startsWith(IPC)) return nativeFetch(input, init);
    const cmd = decodeURIComponent(url.slice(IPC.length));
    const start = now();
    state.inflight += 1;
    touch();
    const settle = () => {
      state.inflight -= 1;
      touch();
      state.calls.push({ cmd, start, end: now() });
    };
    return nativeFetch(input, init).then(
      (response) => (settle(), response),
      (error) => {
        settle();
        throw error;
      },
    );
  };
  // With the custom protocol blocked, Tauri posts every call through chrome.webview and
  // answers by running a callback out of __TAURI_INTERNALS__.callbacks — a plain Map.
  const HEAD = /^\{"cmd":"([^"]+)","callback":(\d+),"error":(\d+)/;
  function patch() {
    const webview = window.chrome?.webview;
    const internals = window.__TAURI_INTERNALS__;
    if (!webview || !internals?.callbacks || webview.__benchPatched) return;
    const post = webview.postMessage.bind(webview);
    webview.postMessage = (message) => {
      const head = typeof message === "string" ? HEAD.exec(message.slice(0, 200)) : null;
      if (head) {
        const [, cmd, ok, fail] = head;
        const start = now();
        state.posted.push({ cmd, at: start });
        if (state.posted.length > 500) state.posted.splice(0, 250);
        state.inflight += 1;
        touch();
        let settled = false;
        const settle = () => {
          if (settled) return;
          settled = true;
          state.inflight -= 1;
          touch();
          state.calls.push({ cmd, start, end: now() });
        };
        for (const id of [Number(ok), Number(fail)]) {
          const original = internals.callbacks.get(id);
          if (original) {
            internals.callbacks.set(id, (data) => {
              settle();
              return original(data);
            });
          }
        }
      }
      return post(message);
    };
    webview.__benchPatched = true;
  }

  // A canvas draw is not a DOM mutation, and the graph is a canvas.
  const draw = CanvasRenderingContext2D.prototype.stroke;
  CanvasRenderingContext2D.prototype.stroke = function (...rest) {
    touch();
    return draw.apply(this, rest);
  };

  const observe = () =>
    new MutationObserver(touch).observe(document.documentElement, {
      subtree: true,
      childList: true,
      attributes: true,
      characterData: true,
    });
  if (document.documentElement) observe();
  else document.addEventListener("DOMContentLoaded", observe, { once: true });

  for (const type of ["mousedown", "keydown", "wheel", "contextmenu", "dblclick"]) {
    window.addEventListener(
      type,
      (event) => {
        if (state.armed && state.input === null) state.input = event.timeStamp;
      },
      { capture: true },
    );
  }

  const frame = () => new Promise((ok) => requestAnimationFrame(() => ok(now())));

  async function idle(quiet, timeout) {
    patch();
    const deadline = now() + timeout;
    for (;;) {
      await frame();
      if (state.inflight === 0 && now() - state.lastActivity >= quiet) return true;
      if (now() > deadline) return false;
    }
  }

  function union(calls, from, to) {
    const spans = calls
      .map((c) => [Math.max(c.start, from), Math.min(c.end, to)])
      .filter(([a, b]) => b > a)
      .sort((a, b) => a[0] - b[0]);
    let total = 0;
    let cursor = -Infinity;
    for (const [a, b] of spans) {
      if (b <= cursor) continue;
      total += b - Math.max(a, cursor);
      cursor = b;
    }
    return total;
  }

  /** probes: name → predicate, checked every frame; the first frame each holds is kept.
      until: a command name — the action is over once the page has sent it (a native menu). */
  async function finish(start, quiet, timeout, { probes = {}, until } = {}) {
    const deadline = start + timeout;
    let painted = null;
    let timedOut = false;
    let end = null;
    const marks = {};
    for (;;) {
      const at = await frame();
      for (const [name, holds] of Object.entries(probes)) {
        if (marks[name] === undefined && holds()) marks[name] = at - start;
      }
      if (until) {
        const sent = state.posted.find((p) => p.cmd === until && p.at >= start);
        if (sent) {
          end = sent.at;
          break;
        }
      }
      const busy = state.inflight > 0 || state.dirty;
      state.dirty = false;
      if (busy) painted = null;
      else if (painted === null) painted = at;
      if (!until && !busy && state.inflight === 0 && at - state.lastActivity >= quiet) break;
      if (at > deadline) {
        timedOut = true;
        break;
      }
    }
    end ??= painted ?? now();
    const calls = state.calls.filter((c) => c.end > start && c.start < end);
    const byCommand = {};
    for (const c of calls) {
      const entry = (byCommand[c.cmd] ??= { count: 0, ms: 0, lastEnd: 0 });
      entry.count += 1;
      entry.ms += c.end - c.start;
      entry.lastEnd = Math.max(entry.lastEnd, c.end - start);
    }
    const total = Math.max(end - start, 0);
    const ipc = union(calls, start, end);
    state.armed = null;
    return { total, ipc, front: Math.max(total - ipc, 0), timedOut, ipcCalls: calls.length, byCommand, marks };
  }

  window.__bench = {
    patch,
    idle: (quiet = 150, timeout = 30000) => idle(quiet, timeout),
    async arm(quiet = 150, timeout = 30000) {
      const settled = await idle(quiet, timeout);
      state.armed = { quiet };
      state.input = null;
      state.calls = [];
      return settled;
    },
    async done(quiet = 150, timeout = 60000, options = {}) {
      const waitFrom = now();
      while (state.input === null) {
        await frame();
        if (now() - waitFrom > 5000) throw new Error("the input event never reached the page");
      }
      return finish(state.input, state.armed?.quiet ?? quiet, timeout, options);
    },
    async run(action, quiet = 150, timeout = 60000, options = {}) {
      await idle(quiet, 30000);
      state.calls = [];
      const start = now();
      state.lastActivity = start;
      state.dirty = true;
      await action();
      return finish(start, quiet, timeout, options);
    },
    find(selector, text) {
      const el = [...document.querySelectorAll(selector)].find((e) => (e.textContent ?? "").includes(text));
      if (!el) return null;
      el.scrollIntoView?.({ block: "nearest" });
      const r = el.getBoundingClientRect();
      return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
    },
    emit: (event, payload) =>
      window.__TAURI_INTERNALS__.invoke("plugin:event|emit", { event, payload }),
    center(selector, index = 0) {
      const all = document.querySelectorAll(selector);
      const el = all[index < 0 ? all.length + index : index];
      if (!el) return null;
      el.scrollIntoView?.({ block: "nearest" });
      const r = el.getBoundingClientRect();
      return { x: r.left + r.width / 2, y: r.top + r.height / 2, count: all.length };
    },
    locate(spec) {
      // One section left (an empty Staged is hidden) means a pane with no heading at all.
      const panes = [...document.querySelectorAll(".pane.tree-rows")];
      const headed = panes.filter((pane) => pane.querySelector(".heading"));
      const roots = !spec.section
        ? [document]
        : headed.length === 0
          ? panes
          : headed.filter((pane) => pane.querySelector(".heading").textContent.trim().startsWith(spec.section));
      let all = roots.flatMap((root) => [...root.querySelectorAll(spec.sel)]);
      if (spec.text !== undefined) {
        all = all.filter((el) => {
          const label = spec.exact ? el.querySelector(spec.exact)?.textContent ?? "" : el.textContent ?? "";
          return spec.exact ? label.trim() === spec.text : label.includes(spec.text);
        });
      }
      const index = spec.index ?? 0;
      let el = all[index < 0 ? all.length + index : index];
      if (el && spec.child) el = el.querySelector(spec.child);
      return el ?? null;
    },
    point(spec) {
      const el = window.__bench.locate(spec);
      if (!el) return null;
      el.scrollIntoView?.({ block: "nearest" });
      const r = el.getBoundingClientRect();
      return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
    },
    /** Fixed work: how much slower than usual the renderer runs right now. */
    calibrate() {
      const start = now();
      let h = 2166136261;
      for (let i = 0; i < 8000000; i += 1) {
        h ^= i;
        h = Math.imul(h, 16777619);
      }
      window.__benchSink = h;
      return now() - start;
    },
    exists: (spec) => window.__bench.locate(spec) !== null,
    attr: (spec, name) => window.__bench.locate(spec)?.getAttribute(name) ?? null,
    state,
  };
})();`;
