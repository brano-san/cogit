<script lang="ts">
  import { onMount, tick, type Snippet } from "svelte";
  import { keyIsFor, modalLayer, modals } from "$lib/modal-stack";

  /** The shell every modal in Cogit is made of: one scrim, one panel, one title bar with
      a close button, one footer. It also owns the look of the controls inside it, so a
      dialog written next month cannot arrive with the platform's own buttons on it
      (doc/12-risks.md, R-108, R-167). */
  interface Props {
    title: string;
    /** Called by the ✕, by Esc and by a click on the scrim. */
    onclose: () => void;
    /** Called by Enter unless the focus is on a button, which then answers for itself. */
    onconfirm?: () => void;
    width?: string;
    /** Fixed height, for a dialog whose content must not make it jump about. */
    height?: string;
    /** Content that draws its own edges: panes, lists with dividers. */
    flush?: boolean;
    children: Snippet;
    footer?: Snippet;
  }

  let {
    title,
    onclose,
    onconfirm,
    width = "min(440px, 90vw)",
    height,
    flush = false,
    children,
    footer,
  }: Props = $props();

  let panel: HTMLDivElement | undefined = $state();
  const layer = modalLayer();
  // Read before anything inside mounts: a dialog that focuses its own field does so first.
  const before = document.activeElement instanceof HTMLElement ? document.activeElement : null;

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
      onclose();
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
<div class="backdrop" onclick={onclose}></div>

<div
  bind:this={panel}
  class="dialog"
  role="dialog"
  aria-modal="true"
  aria-label={title}
  tabindex="-1"
  style:width
  style:height
>
  <header>
    <h2>{title}</h2>
    <button type="button" class="close" onclick={onclose} aria-label="Close" title="Close (Esc)">
      <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1 9 9M9 1 1 9" /></svg>
    </button>
  </header>

  <div class="body" class:flush>{@render children()}</div>

  {#if footer}
    <footer>{@render footer()}</footer>
  {/if}
</div>

<style>
  .backdrop {
    position: absolute;
    inset: 0;
    z-index: 20;
    background: var(--scrim);
  }

  .dialog {
    --dialog-inset: var(--sp-5);
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 21;
    display: flex;
    flex-direction: column;
    max-height: 88vh;
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
    margin: 0;
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
    overflow: auto;
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
    padding: var(--sp-5) var(--dialog-inset);
    border-top: 1px solid var(--divider);
  }

  /* Opt-in, by class, never by element: `.dialog :global(button)` outweighed every
     component's own button rules and boxed them all (doc/12-risks.md, R-125). */
  .dialog :global(.btn) {
    height: var(--h-button);
    padding: 0 var(--sp-5);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font: inherit;
    font-size: var(--fs-dense);
    cursor: default;
  }

  .dialog :global(.btn:hover:not(:disabled)) {
    border-color: var(--state-focus-ring);
  }

  .dialog :global(.btn:focus) {
    outline: 1px solid var(--state-focus-ring);
    outline-offset: 1px;
  }

  .dialog :global(.btn:disabled) {
    color: var(--text-secondary);
    opacity: 0.6;
  }

  .dialog :global(.btn.primary),
  .dialog :global(.btn.warning) {
    background: var(--status-ref);
    border-color: var(--status-ref);
    color: var(--surface-base);
    font-weight: 600;
  }

  .dialog :global(.btn.warning) {
    background: var(--status-modify);
    border-color: var(--status-modify);
  }

  .dialog :global(.btn.primary:hover:not(:disabled)),
  .dialog :global(.btn.warning:hover:not(:disabled)) {
    filter: brightness(1.1);
  }

  .dialog :global(.btn.primary:disabled),
  .dialog :global(.btn.warning:disabled) {
    background: var(--surface-input);
    border-color: var(--field-border);
    color: var(--text-secondary);
    font-weight: 400;
  }

  .dialog :global(input[type="text"]),
  .dialog :global(input[type="search"]),
  .dialog :global(input[type="password"]) {
    width: 100%;
    height: var(--h-input);
    padding: 0 var(--sp-4);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font: inherit;
    font-size: var(--fs-dense);
  }

  .dialog :global(input[type="checkbox"]:not(.native)),
  .dialog :global(input[type="radio"]:not(.native)) {
    accent-color: var(--status-ref);
    width: 13px;
    height: 13px;
    margin: 0;
  }
</style>
