import { describe, expect, it, vi } from "vitest";
import { aiConfig, knownAiConfig } from "$lib/ai-config";
import type { AiChunk, AiKeyState, AiMessage } from "$lib/ipc";
import { AiMessageStore, generateMessageBlocker, type Generation } from "./ai-message.svelte";

const config = aiConfig("openai");
const key: AiKeyState = {
  state: "unlocked", stored: true, required: true, host: "api.openai.com", service: "https://api.openai.com/v1",
};

function harness(initial = "", prefix: string | null = null) {
  let text = initial;
  let valid = true;
  let receive: (chunk: AiChunk) => void = () => {};
  let resolve: (message: AiMessage) => void = () => {};
  const remembered = new Set<string>();
  const deps = {
    consent: vi.fn(async () => ({ yes: true, checked: true })),
    replace: vi.fn(async () => true),
    allowed: vi.fn(async (id: string) => remembered.has(id)),
    remember: vi.fn(async (id: string) => { remembered.add(id); }),
    cancel: vi.fn(async (_id: number) => {}),
    generate: vi.fn((_repo, _request, onChunk: (chunk: AiChunk) => void) => {
      receive = onChunk;
      return new Promise<AiMessage>((done) => { resolve = done; });
    }),
    report: vi.fn(),
  };
  const store = new AiMessageStore(deps);
  const input: Generation = {
    config, key, repo: 1, name: "fixture", prefix,
    read: () => text,
    write: (next) => { text = next; },
    valid: () => valid,
    ownsField: () => true,
  };
  return {
    deps, store, input,
    text: () => text,
    invalidate: () => { valid = false; },
    push: (chunk: AiChunk) => receive(chunk),
    finish: (message = "Add a preview") => resolve({ message, shortened: false }),
    pending: () => resolve,
  };
}

describe("AI commit message", () => {
  it("needs a configured provider, staged files, and an open store when a key is needed", () => {
    expect(generateMessageBlocker(0, config, key, false)).toMatch(/stage/i);
    expect(generateMessageBlocker(1, null, null, false)).toMatch(/Preferences/);
    expect(generateMessageBlocker(1, config, null, false)).toMatch(/checking/i);
    expect(generateMessageBlocker(1, config, { ...key, state: "locked" }, false)).toMatch(/locked/i);
    expect(generateMessageBlocker(1, config, { ...key, stored: false }, false)).toMatch(/API key/);
    expect(generateMessageBlocker(1, config, { ...key, state: "missing", required: false, stored: false }, false)).toBeUndefined();
    expect(generateMessageBlocker(1, config, key, true)).toMatch(/progress/);
  });

  it("stores only the public provider configuration", () => {
    expect(knownAiConfig({ ...config, key: "secret", apiKey: "secret" })).toEqual(config);
    expect(knownAiConfig({ ...config, baseUrl: "https://user:secret@api.openai.com/v1" })).toBeNull();
    expect(knownAiConfig({ ...config, baseUrl: "https://api.openai.com/v1?api_key=secret" })).toBeNull();
  });

  it("remembers consent only for the selected provider and base URL", async () => {
    const h = harness();
    async function run(input = h.input) {
      const pending = h.store.start(input);
      await vi.waitFor(() => expect(h.store.streaming).toBe(true));
      h.finish();
      await pending;
    }
    await run();
    await run();
    expect(h.deps.consent).toHaveBeenCalledTimes(1);
    await run({ ...h.input, key: { ...key, service: "https://example.test/v1" } });
    await run({ ...h.input, config: aiConfig("anthropic") });
    expect(h.deps.consent).toHaveBeenCalledTimes(3);
  });

  it("sends nothing if consent is declined", async () => {
    const h = harness();
    h.deps.consent.mockResolvedValueOnce({ yes: false, checked: false });
    await h.store.start(h.input);
    expect(h.deps.generate).not.toHaveBeenCalled();
    expect(h.deps.remember).not.toHaveBeenCalled();
  });

  it("asks before replacing a draft and preserves it if declined", async () => {
    const h = harness("Keep my draft");
    h.deps.replace.mockResolvedValueOnce(false);
    await h.store.start(h.input);
    expect(h.deps.replace).toHaveBeenCalledOnce();
    expect(h.deps.generate).not.toHaveBeenCalled();
    expect(h.text()).toBe("Keep my draft");
  });

  it("appends streamed text and applies the branch prefix to the completed message", async () => {
    const h = harness("ABC-1: ", "ABC-1: ");
    const pending = h.store.start(h.input);
    await vi.waitFor(() => expect(h.store.streaming).toBe(true));
    h.push({ kind: "text", text: "Add " });
    expect(h.text()).toBe("Add ");
    h.push({ kind: "text", text: "a preview" });
    expect(h.text()).toBe("Add a preview");
    h.finish();
    await pending;
    expect(h.text()).toBe("ABC-1: Add a preview");
    expect(h.deps.replace).not.toHaveBeenCalled();
  });

  it("cancels even before the operation ID arrives and restores the previous draft", async () => {
    const h = harness("Original");
    const pending = h.store.start(h.input);
    await vi.waitFor(() => expect(h.store.streaming).toBe(true));
    h.push({ kind: "text", text: "Partial" });
    h.store.cancel();
    h.push({ kind: "started", id: 19 });
    h.push({ kind: "text", text: " late" });
    h.finish();
    await pending;
    expect(h.deps.cancel).toHaveBeenCalledWith(19);
    expect(h.text()).toBe("Original");
    expect(h.store.active).toBe(false);
  });

  it("does not start when the repository changes during consent", async () => {
    const h = harness();
    h.deps.consent.mockImplementationOnce(async () => {
      h.invalidate();
      return { yes: true, checked: true };
    });
    await h.store.start(h.input);
    expect(h.deps.generate).not.toHaveBeenCalled();
    expect(h.deps.remember).not.toHaveBeenCalled();
  });

  it("does not let a canceled reply replace text from a retry", async () => {
    const h = harness("Original");
    const canceled = h.store.start(h.input);
    await vi.waitFor(() => expect(h.store.streaming).toBe(true));
    const lateReply = h.pending();
    h.push({ kind: "text", text: "Add " });
    h.store.cancel();
    const retried = h.store.start(h.input);
    await vi.waitFor(() => expect(h.store.streaming).toBe(true));
    h.push({ kind: "text", text: "Add " });
    lateReply({ message: "Old reply", shortened: false });
    await canceled;
    expect(h.text()).toBe("Add ");
    h.finish();
    await retried;
    expect(h.text()).toBe("Add a preview");
  });
});
