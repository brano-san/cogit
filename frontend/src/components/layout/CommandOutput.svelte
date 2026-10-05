<script lang="ts">
  import { NO_PROMPT_ADVICE, credentialTrouble } from "$lib/credentials";
  import VirtualList from "$components/common/VirtualList.svelte";
  import { CopyFeedback } from "$lib/copy-feedback.svelte";
  import { splitLinks } from "$lib/links";
  import { revealOnDesktop } from "$lib/ipc/file-menus";
  import { asCogitError, commandReport, repoNameOf } from "$lib/notices";
  import { findMatches, logLines } from "$lib/output-highlight";
  import { outputKey } from "$lib/output-keys";
  import { ON_MAC, primary } from "$lib/platform";
  import { readKey, writeKey } from "$lib/settings-file";
  import { startsDrag } from "$lib/error-window";
  import { clampBox, defaultBox, type Box } from "$lib/window-box";
  import type { Snippet } from "svelte";
  import type { GitOutput } from "$lib/ipc";
  import { openExternal } from "$lib/ipc/file-menus";
  import { errors } from "$stores/errors.svelte";
  import { output } from "$stores/output.svelte";

  interface Props {
    entry: GitOutput;
    /** Where the untrimmed copy of this output went. */
    logPath: string;
    /** Absent when the operation is not one that can simply be run again. */
    onretry?: (() => void) | undefined;
    /** Fills the window it is in instead of floating over the page: the Errors window. */
    docked?: boolean;
    /** Where ✕, Close and Esc go; the floating window closes itself in the output store. */
    onclose?: (() => void) | undefined;
    /** Title and accent when the record is not simply a failure ("Stash applied with conflicts"). */
    heading?: string | undefined;
    warned?: boolean | undefined;
    /** Buttons of the caller, in the footer before Copy output. */
    actions?: Snippet | undefined;
  }

  let { entry, logPath, onretry, docked = false, onclose, heading: given, warned: accent, actions }: Props =
    $props();

  function close() {
    if (onclose) onclose();
    else output.close();
  }

  const SETTINGS_KEY = "outputWindow";
  const ROW = 18;
  const FONT = { min: 10, max: 20, step: 1 };
  const WRAP_MAX = 5_000;

  const lines = $derived(logLines(entry.stdout, entry.stderr));
  /** Above the raw output, never instead of it: what to do when no credentials could be asked for. */
  const noPrompt = $derived(credentialTrouble(`${entry.stderr}\n${entry.stdout}`) === "noPrompt");
  const failed = $derived(accent === undefined ? entry.severity === "failure" : !accent);
  const heading = $derived(
    given ??
      (entry.severity === "failure"
        ? `${entry.operation} failed`
        : `${entry.operation} finished with warnings`),
  );
  const repoName = $derived(repoNameOf(entry.repo));

  let box = $state<Box | null>(null);
  let font = $state(12);
  let wrap = $state(false);
  let allSelected = $state(false);
  let finding = $state(false);
  let needle = $state("");
  let at = $state(0);
  const feedback = new CopyFeedback();
  let findInput: HTMLInputElement | undefined = $state();
  let closer: HTMLButtonElement | undefined = $state();
  let frame: HTMLElement | undefined = $state();

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
    if (docked) return;
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
    if (docked || !box || !startsDrag(event)) return;
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
    await feedback.copy(picked !== "" && !allSelected ? picked : commandReport(entry));
  }

  function resize(by: number) {
    font = Math.min(Math.max(font + by, FONT.min), FONT.max);
    remember();
  }

  function onkeydown(event: KeyboardEvent) {
    const action = outputKey({
      key: event.key,
      code: event.code,
      ctrl: primary(event, ON_MAC),
      inside: frame !== undefined && event.target instanceof Node && frame.contains(event.target),
      handled: event.defaultPrevented,
      finding,
      allSelected,
    });
    if (action === null) return;
    if (action === "end-find") {
      finding = false;
      needle = "";
      return;
    }
    if (action === "close") {
      if (docked) event.preventDefault();
      close();
      return;
    }
    event.preventDefault();
    if (action === "find") {
      finding = true;
      queueMicrotask(() => findInput?.select());
    } else if (action === "select-all") allSelected = true;
    else if (action === "copy") void copy();
    else resize(action === "bigger" ? FONT.step : -FONT.step);
  }

  let logProblem = $state("");

  async function openLog() {
    logProblem = "";
    await revealOnDesktop(logPath).catch((err) => {
      logProblem = asCogitError(err)?.message ?? String(err);
    });
  }

  /** A link git printed (the pull request a push offers) opens in the browser (F-028). */
  async function openLink(event: MouseEvent, href: string) {
    event.preventDefault();
    await openExternal(href).catch((err) => errors.report(err, "Could not open the link"));
  }

  function step(by: number) {
    if (hits.length === 0) return;
    at = (at + by + hits.length) % hits.length;
  }
</script>

<svelte:window {onkeydown} />

<!-- On one line: the rows keep their whitespace, so a line break here would show. -->
{#snippet linked(text: string)}{#if text.includes("://")}{#each splitLinks(text) as part, index (index)}{#if part.href}<a class="url" href={part.href} title={part.href} onclick={(event) => void openLink(event, part.href ?? "")}>{part.text}</a>{:else}{part.text}{/if}{/each}{:else}{text || " "}{/if}{/snippet}

{#if box || docked}
  <!-- Non-modal by choice: the reader compares the output against the graph and the
       files while it is up (doc/12-risks.md, R-89). -->
  <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
  <!-- Focusable, so a click in the output keeps its keys here and not on the page. -->
  <section
    bind:this={frame}
    data-select-own
    class="window"
    class:warned={!failed}
    class:docked
    role="dialog"
    tabindex="-1"
    aria-label={heading}
    style:left={box ? `${box.x}px` : undefined}
    style:top={box ? `${box.y}px` : undefined}
    style:width={box ? `${box.w}px` : undefined}
    style:height={box ? `${box.h}px` : undefined}
  >
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <header onpointerdown={(e) => grab(e, "move")}>
      <span class="dot" aria-hidden="true"></span>
      <span class="title">{docked ? heading : entry.operation} · {repoName}</span>
      <span class="grow"></span>
      <!-- Docked, it fills the Errors window, whose title bar has the close button already. -->
      {#if !docked}
        <button bind:this={closer} type="button" class="btn sm" onclick={close} title="Close (Esc)">
          ✕
        </button>
      {/if}
    </header>

    <div class="body">
      <dl class="facts">
        <dt>Repository</dt>
        <dd class="mono" title={entry.repo}>{repoName}</dd>
        <dt>Command</dt>
        <dd class="mono">{entry.command}</dd>
        <dt>Exit code</dt>
        <dd class="mono tabular">{entry.exitCode ?? "did not start"}</dd>
        <dt>Duration</dt>
        <dd class="tabular">{entry.durationMs} ms</dd>
        <dt>Started</dt>
        <dd class="tabular">{new Date(entry.startedAtMs).toLocaleString()}</dd>
      </dl>

      {#if noPrompt}
        <p class="advice" role="note">{NO_PROMPT_ADVICE}</p>
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
          <button type="button" class="btn sm" onclick={() => step(-1)} title="Previous (Shift+Enter)">↑</button>
          <button type="button" class="btn sm" onclick={() => step(1)} title="Next (Enter)">↓</button>
          <button type="button" class="btn sm" onclick={() => ((finding = false), (needle = ""))}>✕</button>
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
                {@render linked(line.text)}
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
                {@render linked(line.text)}
              </div>
            {/snippet}
          </VirtualList>
        {/if}
      </div>
    </div>

    <footer>
      <button
        type="button"
        class="btn sm"
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
      <button type="button" class="btn sm" onclick={() => void openLog()} title={logPath}>Open log</button>
      {#if logProblem}<span class="problem truncate" title={logProblem}>{logProblem}</span>{/if}
      <span class="grow"></span>
      {#if actions}<span class="extra">{@render actions()}</span>{/if}
      {#if onretry}
        <button type="button" class="btn sm" onclick={onretry}>Retry</button>
      {/if}
      <button type="button" class="btn sm" onclick={copy}>{feedback.label("Copy output")}</button>
      <button type="button" class="btn sm primary" onclick={close}>Close</button>
    </footer>

    <!-- svelte-ignore a11y_no_static_element_interactions -->
    {#if !docked}
      <div class="grip" onpointerdown={(e) => grab(e, "size")}></div>
    {/if}
  </section>
{/if}

<style>
  .advice {
    margin: 0 0 var(--sp-4);
    padding: var(--sp-3) var(--sp-4);
    background: var(--badge-warning-bg);
    color: var(--badge-warning-fg);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    user-select: text;
  }

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

  .window.docked {
    position: relative;
    z-index: auto;
    width: 100%;
    height: 100%;
    border: 0;
    border-radius: 0;
    box-shadow: none;
  }

  .docked header {
    cursor: default;
    border-radius: 0;
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

  .problem {
    min-width: 0;
    color: var(--status-delete);
  }

  .body {
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-height: 0;
    padding: var(--sp-4) var(--sp-4) 0;
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

  .url {
    color: var(--link);
  }

  .ln.flow {
    position: static;
    height: auto;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .extra {
    display: contents;
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
    background: var(--accent);
    color: var(--fg-on-accent);
  }

  .picked .ln {
    background: var(--bg-selected);
  }

  footer {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex: 0 0 auto;
    padding: var(--sp-4);
  }

  .on {
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
