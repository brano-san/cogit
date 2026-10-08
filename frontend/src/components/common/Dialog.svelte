<script lang="ts">
  import { onMount, tick, type Snippet } from "svelte";
  import ConfirmDialog from "$components/common/ConfirmDialog.svelte";
  import { keyIsFor, modalLayer, modals } from "$lib/modal-stack";
  import { closeAnswer, type CloseRequest } from "$lib/unsaved";
  import { taskbar } from "$stores/taskbar.svelte";

  /** The shell every modal in Cogit is made of: one scrim, one panel, one title bar with
      a close button, one footer. It also owns the look of the controls inside it, so a
      dialog written next month cannot arrive with the platform's own buttons on it
      (doc/12-risks.md, R-108, R-167). */
  interface Props {
    title: string;
    /** Called by the ✕, by Esc and by a click on the scrim. */
    onclose: () => void;
    /** Typed work the dialog would lose: the scrim then leaves it open, Esc and the ✕ ask
        first, and closing the window names it (R-515). */
    dirty?: boolean;
    /** Called by Enter unless the focus is on a button, which then answers for itself. */
    onconfirm?: () => void;
    width?: string;
    /** A background token for the panel, for a dialog that sits on the elevated surface. */
    surface?: string;
    /** Fixed height, for a dialog whose content must not make it jump about. */
    height?: string;
    /** Content that draws its own edges: panes, lists with dividers. */
    flush?: boolean;
    /** Side padding of the title bar, body and footer (default `--sp-5`). */
    inset?: string;
    children: Snippet;
    footer?: Snippet;
  }

  let {
    title,
    onclose,
    onconfirm,
    dirty = false,
    width = "min(440px, 90vw)",
    surface,
    height,
    flush = false,
    inset,
    children,
    footer,
  }: Props = $props();

  let panel: HTMLDivElement | undefined = $state();
  const layer = modalLayer();
  // Read before anything inside mounts: a dialog that focuses its own field does so first.
  const before = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  // By depth, not by place in the page: the question a dialog raises is mounted anywhere.
  // The first layer keeps the 20 and 21 it always had, above the corner toast.
  const zIndex = 20 + 2 * modals.depth(layer);

  /** "Discard Changes" is showing over the dialog. */
  let asking = $state(false);

  $effect(() => modals.markUnsaved(layer, dirty ? title : null));

  function requestClose(request: CloseRequest) {
    const answer = closeAnswer(request, dirty);
    if (answer === "close") onclose();
    else if (answer === "ask") asking = true;
  }

  const FOCUSABLE =
    'button:not(:disabled), [href], input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [tabindex]:not([tabindex="-1"])';

  function focusables(): HTMLElement[] {
    if (!panel) return [];
    return [...panel.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
      (element) => element.getClientRects().length > 0,
    );
  }

  function wrapTab(event: KeyboardEvent) {
    const list = focusables();
    const first = list[0];
    const last = list.at(-1);
    if (!first || !last || !panel?.contains(document.activeElement)) return;
    const edge = event.shiftKey ? first : last;
    if (document.activeElement !== edge && document.activeElement !== panel) return;
    event.preventDefault();
    (event.shiftKey ? last : first).focus();
  }

  /** Only the top modal answers, and only for a key pressed in it: every dialog listens on
      the window, and Esc closed a dialog and the Hooks window under it at once (R-451). */
  function onkeydown(event: KeyboardEvent) {
    if (!modals.isTop(layer) || !keyIsFor(panel, event.target)) return;
    if (event.key === "Escape") {
      if (event.defaultPrevented) return;
      event.preventDefault();
      requestClose("escape");
      return;
    }
    if (event.key === "Tab") {
      wrapTab(event);
      return;
    }
    if (event.key !== "Enter" || !onconfirm || event.isComposing || event.defaultPrevented) return;
    const target = event.target;
    // Enter in a textarea is a newline; on a button it presses that button.
    if (target instanceof HTMLTextAreaElement) return;
    if (target instanceof HTMLButtonElement || target instanceof HTMLAnchorElement) return;
    event.preventDefault();
    onconfirm();
  }

  onMount(() => {
    // A question asked behind the user's back waits for them: the taskbar button says so.
    taskbar.attention();
    void tick().then(() => {
      if (!panel || panel.contains(document.activeElement)) return;
      (panel.querySelector<HTMLElement>("[data-autofocus]") ?? panel).focus();
    });
    return () => {
      if (before?.isConnected) before.focus();
    };
  });
</script>

<svelte:window {onkeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" style:z-index={zIndex} onclick={() => requestClose("scrim")}></div>

<div
  bind:this={panel}
  class="dialog"
  role="dialog"
  aria-modal="true"
  aria-label={title}
  tabindex="-1"
  style:width
  style:height
  style:background={surface}
  style:--dialog-inset={inset}
  style:z-index={zIndex + 1}
>
  <header>
    <h2 {title}>{title}</h2>
    <button type="button" class="close" onclick={() => requestClose("button")} aria-label="Close" title="Close (Esc)">
      <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1 9 9M9 1 1 9" /></svg>
    </button>
  </header>

  <div class="body" class:flush>{@render children()}</div>

  {#if footer}
    <footer>{@render footer()}</footer>
  {/if}
</div>

{#if asking}
  <ConfirmDialog
    title="Discard Changes"
    message="What you changed in “{title}” has not been saved. Discard it?"
    confirm="Discard"
    warning
    onanswer={(yes) => {
      asking = false;
      if (yes) onclose();
    }}
  />
{/if}

<style>
  .backdrop {
    position: absolute;
    inset: 0;
    background: var(--scrim);
    opacity: var(--scrim-opacity);
  }

  .dialog {
    --dialog-inset: var(--sp-5);
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    display: flex;
    flex-direction: column;
    max-width: 90vw;
    max-height: 90vh;
    background: var(--surface-panel);
    border: 1px solid var(--field-border);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-popover);
    overflow: hidden;
  }

  .dialog:focus-visible {
    outline: none;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-4);
    flex: 0 0 auto;
    height: var(--h-toolbar);
    padding: 0 var(--sp-3) 0 var(--dialog-inset);
    background: var(--titlebar-bg);
    border-bottom: 1px solid var(--titlebar-border);
  }

  h2 {
    min-width: 0;
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-ui);
    font-weight: 600;
  }

  .close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 auto;
    width: 26px;
    height: 26px;
    padding: 0;
    background: none;
    border: 0;
    border-radius: var(--r-sm);
    color: var(--text-secondary);
    cursor: default;
  }

  .close:hover {
    background: var(--state-hover);
    color: var(--text-primary);
  }

  .close:active {
    background: var(--state-selected);
  }

  .close svg {
    width: 10px;
    height: 10px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linecap: round;
  }

  .body {
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-height: 0;
    padding: var(--sp-6) var(--dialog-inset);
    /* Never a horizontal scrollbar: a row wraps or truncates, it does not push the edge.
       The body scrolls vertically; the title bar and the footer stay where they are. */
    overflow-x: hidden;
    overflow-y: auto;
    /* A long unbroken name (a path, a branch) wraps inside its row instead of widening it. */
    overflow-wrap: anywhere;
  }

  .body > :global(*) {
    min-width: 0;
    max-width: 100%;
  }

  .body.flush {
    padding: 0;
  }

  footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--sp-4);
    flex: 0 0 auto;
    flex-wrap: wrap;
    min-width: 0;
    padding: var(--sp-5) var(--dialog-inset);
    border-top: 1px solid var(--divider);
  }

  .dialog :global(input[type="text"]),
  .dialog :global(input[type="search"]),
  .dialog :global(input[type="password"]) {
    width: 100%;
    max-width: 100%;
    box-sizing: border-box;
    height: var(--h-input);
    padding: 0 var(--sp-4);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font: inherit;
    font-size: var(--fs-dense);
  }

  .dialog :global(input:disabled) {
    color: var(--fg-disabled);
    opacity: 0.6;
  }

  .dialog :global(input[type="checkbox"]:not(.native)),
  .dialog :global(input[type="radio"]:not(.native)) {
    accent-color: var(--status-ref);
    width: 13px;
    height: 13px;
    margin: 0;
  }
</style>
