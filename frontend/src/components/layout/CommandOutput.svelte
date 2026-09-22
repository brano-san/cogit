<script lang="ts">
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import VirtualList from "$components/common/VirtualList.svelte";
  import { commandReport, repoNameOf } from "$lib/notices";
  import { findMatches, logLines } from "$lib/output-highlight";
  import { readKey, writeKey } from "$lib/settings-file";
  import { clampBox, defaultBox, type Box } from "$lib/window-box";
  import type { GitOutput } from "$lib/ipc";
  import { output } from "$stores/output.svelte";

  interface Props {
    entry: GitOutput;
    /** Where the untrimmed copy of this output went. */
    logPath: string;
    /** Absent when the operation is not one that can simply be run again. */
    onretry?: (() => void) | undefined;
  }

  let { entry, logPath, onretry }: Props = $props();

  const SETTINGS_KEY = "outputWindow";
  const ROW = 18;
  const FONT = { min: 10, max: 20, step: 1 };
  const WRAP_MAX = 5_000;

  const lines = $derived(logLines(entry.stdout, entry.stderr));
  const failed = $derived(entry.severity === "failure");
  const heading = $derived(
    failed ? `${entry.operation} failed` : `${entry.operation} finished with warnings`,
  );
  const repoName = $derived(repoNameOf(entry.repo));

  let box = $state<Box | null>(null);
  let font = $state(12);
  const technical = true;
  let wrap = $state(false);
  let allSelected = $state(false);
  let finding = $state(false);
  let needle = $state("");
  let at = $state(0);
  let copied = $state(false);
  let findInput: HTMLInputElement | undefined = $state();
  let closer: HTMLButtonElement | undefined = $state();

  const hits = $derived(finding ? findMatches(lines, needle) : []);
  const hitSet = $derived(new Set(hits));
  const cursor = $derived(hits.length === 0 ? null : (hits[at % hits.length] ?? null));
  /** Wrapping gives every line its own height, which is the one thing virtualization
      cannot have. Past this many lines the toggle stays off rather than risk the DOM. */
  const canWrap = $derived(lines.length <= WRAP_MAX);

  function viewport() {
    return { width: window.innerWidth, height: window.innerHeight };
  }

  $effect(() => {
    void (async () => {
      const saved = await readKey<Box & { font?: number }>(SETTINGS_KEY);
      box = saved ? clampBox(saved, viewport()) : defaultBox(viewport());
      if (saved?.font) font = saved.font;
    })();
  });

  $effect(() => {
    closer?.focus();
  });

  function remember() {
    if (box) void writeKey(SETTINGS_KEY, { ...box, font });
  }

  /** Pointer capture, not a document listener: a drag that leaves the window still ends. */
  function grab(event: PointerEvent, edge: "move" | "size") {
    if (!box || event.button !== 0) return;
    const start = { ...box, px: event.clientX, py: event.clientY };
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(event.pointerId);

    const move = (e: PointerEvent) => {
      const dx = e.clientX - start.px;
      const dy = e.clientY - start.py;
      const moved =
        edge === "move"
          ? { ...start, x: start.x + dx, y: start.y + dy }
          : { ...start, w: start.w + dx, h: start.h + dy };
      box = clampBox(moved, viewport());
    };
    const drop = () => {
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", drop);
      remember();
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", drop);
  }

  async function copy() {
    const picked = window.getSelection()?.toString() ?? "";
    await writeText(picked !== "" && !allSelected ? picked : commandReport(entry));
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  function resize(by: number) {
    font = Math.min(Math.max(font + by, FONT.min), FONT.max);
    remember();
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      if (finding) {
        finding = false;
        needle = "";
        return;
      }
      output.close();
      return;
    }
    if (!event.ctrlKey && !event.metaKey) return;

    switch (event.key) {
      case "f":
        event.preventDefault();
        finding = true;
        queueMicrotask(() => findInput?.select());
        break;
      case "a":
        event.preventDefault();
        allSelected = true;
        break;
      case "c":
        if (allSelected) {
          event.preventDefault();
          void copy();
        }
        break;
      case "=":
      case "+":
        event.preventDefault();
        resize(FONT.step);
        break;
      case "-":
        event.preventDefault();
        resize(-FONT.step);
        break;
    }
  }

  /** Imported here rather than at the top: the opener is one call on one button, and a
      static import pulls the whole plugin into the first chunk the window is in. */
  async function openLog() {
    const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
    await revealItemInDir(logPath).catch(() => {});
  }

  function step(by: number) {
    if (hits.length === 0) return;
    at = (at + by + hits.length) % hits.length;
  }
</script>

<svelte:window {onkeydown} />

{#if box}
  <!-- Non-modal by choice: the reader compares the output against the graph and the
       files while it is up (doc/12-risks.md, R-89). -->
  <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
  <section
    class="window"
    class:warned={!failed}
    role="dialog"
    aria-label={heading}
    style:left="{box.x}px"
    style:top="{box.y}px"
    style:width="{box.w}px"
    style:height="{box.h}px"
  >
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <header onpointerdown={(e) => grab(e, "move")}>
      <span class="dot" aria-hidden="true"></span>
      <span class="title">{entry.operation} · {repoName}</span>
      <span class="grow"></span>
      <button bind:this={closer} type="button" onclick={() => output.close()} title="Close (Esc)">
        ✕
      </button>
    </header>

    <div class="body">
      {#if technical}
        <dl class="facts">
          <dt>Command</dt>
          <dd class="mono">{entry.command}</dd>
          <dt>Exit code</dt>
          <dd class="mono tabular">{entry.exitCode ?? "did not start"}</dd>
          <dt>Duration</dt>
          <dd class="tabular">{entry.durationMs} ms</dd>
          <dt>Started</dt>
          <dd class="tabular">{new Date(entry.startedAtMs).toLocaleString()}</dd>
        </dl>
      {/if}

      {#if finding}
        <div class="find">
          <input
            bind:this={findInput}
            bind:value={needle}
            type="search"
            placeholder="Find in output"
            oninput={() => (at = 0)}
            onkeydown={(e) => e.key === "Enter" && step(e.shiftKey ? -1 : 1)}
          />
          <span class="count tabular">
            {hits.length === 0 ? (needle === "" ? "" : "no matches") : `${(at % hits.length) + 1} of ${hits.length}`}
          </span>
          <button type="button" onclick={() => step(-1)} title="Previous (Shift+Enter)">↑</button>
          <button type="button" onclick={() => step(1)} title="Next (Enter)">↓</button>
          <button type="button" onclick={() => ((finding = false), (needle = ""))}>✕</button>
        </div>
      {/if}

      <!-- Virtualized: a hook can print a hundred thousand lines and the DOM gets ~50
           of them (doc/12-risks.md, R-90). -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="out"
        class:picked={allSelected}
        style:font-size="{font}px"
        onpointerdown={() => (allSelected = false)}
      >
        {#if wrap && canWrap}
          <div class="flowed" role="list" aria-label="Output">
            {#each lines as line, index (index)}
              <div class="ln flow {line.kind}" class:hit={hitSet.has(index)} class:cursor={index === cursor}>
                {line.text || " "}
              </div>
            {/each}
          </div>
        {:else}
          <VirtualList items={lines} rowHeight={ROW} buffer={20} reveal={cursor} label="Output">
            {#snippet row(line, index)}
              <div
                class="ln {line.kind}"
                class:hit={hitSet.has(index)}
                class:cursor={index === cursor}
                style:top="{index * ROW}px"
              >
                {line.text || " "}
              </div>
            {/snippet}
          </VirtualList>
        {/if}
      </div>
    </div>

    <footer>
      <button
        type="button"
        onclick={() => (wrap = !wrap)}
        class:on={wrap}
        aria-pressed={wrap}
        disabled={!canWrap}
        title={canWrap
          ? "Off by default: compiler output is unreadable wrapped."
          : "Too many lines to wrap; virtualization needs every line the same height."}
      >
        Wrap lines
      </button>
      <button type="button" onclick={() => void openLog()} title={logPath}>Open log</button>
      <span class="grow"></span>
      {#if onretry}
        <button type="button" onclick={onretry}>Retry</button>
      {/if}
      <button type="button" onclick={copy}>{copied ? "Copied" : "Copy output"}</button>
      <button type="button" class="primary" onclick={() => output.close()}>Close</button>
    </footer>

    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="grip" onpointerdown={(e) => grab(e, "size")}></div>
  </section>
{/if}

<style>
  .window {
    position: fixed;
    z-index: 30;
    display: flex;
    flex-direction: column;
    background: var(--surface-panel);
    border: 1px solid var(--status-delete);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-dialog);
    font-size: var(--fs-dense);
  }

  .window.warned {
    border-color: var(--status-modify);
  }

  header {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex: 0 0 auto;
    height: var(--h-panel-hdr);
    padding: 0 var(--sp-4);
    background: var(--surface-raised);
    border-bottom: 1px solid var(--divider);
    border-radius: var(--r-md) var(--r-md) 0 0;
    cursor: move;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--status-delete);
  }

  .warned .dot {
    background: var(--status-modify);
  }

  .title {
    font-weight: 600;
  }

  .grow {
    flex: 1 1 auto;
  }

  .body {
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-height: 0;
    padding: var(--sp-4) var(--sp-4) 0;
  }

  .technical,
  .more {
    align-self: flex-start;
    padding: 0 var(--sp-2);
    background: none;
    border: 0;
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }

  .more {
    color: var(--status-delete);
  }

  .technical:hover,
  .more:hover {
    color: var(--text-primary);
  }

  .facts {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--sp-2) var(--sp-4);
    margin: var(--sp-3) 0;
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }

  .facts dd {
    margin: 0;
    color: var(--text-primary);
    user-select: text;
    overflow-wrap: anywhere;
  }

  .find {
    display: flex;
    align-items: center;
    gap: var(--sp-2);
    margin: var(--sp-3) 0 var(--sp-2);
  }

  .find input {
    flex: 1 1 auto;
    min-width: 0;
    height: var(--h-button-sm);
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
  }

  .count {
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }

  .out {
    flex: 1 1 auto;
    min-height: 0;
    margin: var(--sp-3) 0 0;
    background: var(--surface-input);
    border: 1px solid var(--divider);
    border-radius: var(--r-sm);
    display: flex;
    user-select: text;
    cursor: text;
  }

  .out.picked {
    outline: 1px solid var(--status-ref);
  }

  /* Raw output is never reformatted (INV-05); wrapping is a choice, not the default,
     because a wrapped stack trace is unreadable. */
  .ln {
    position: absolute;
    left: 0;
    padding: 0 var(--sp-3);
    height: 18px;
    line-height: 18px;
    white-space: pre;
    font-family: var(--font-mono);
    user-select: text;
  }

  .flowed {
    flex: 1 1 auto;
    min-width: 0;
    overflow: auto;
  }

  .ln.flow {
    position: static;
    height: auto;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  button:disabled {
    color: var(--text-secondary);
    border-color: transparent;
  }

  .ln.error {
    color: var(--status-delete);
  }

  .ln.warning {
    color: var(--status-modify);
  }

  .ln.omitted {
    color: var(--text-secondary);
    font-style: italic;
  }

  .ln.label {
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .ln.hit {
    background: var(--surface-raised);
  }

  .ln.cursor {
    background: var(--status-ref);
    color: var(--surface-panel);
  }

  .picked .ln {
    background: color-mix(in srgb, var(--status-ref) 22%, transparent);
  }

  footer {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex: 0 0 auto;
    padding: var(--sp-4);
  }

  button {
    height: var(--h-button-sm);
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }

  button:hover {
    border-color: var(--status-ref);
  }

  button.on,
  button.primary {
    border-color: var(--status-ref);
  }

  .grip {
    position: absolute;
    right: 0;
    bottom: 0;
    width: 16px;
    height: 16px;
    cursor: nwse-resize;
  }
</style>
