<script lang="ts">
  import type { Snippet } from "svelte";

  /** The shell every modal in Cogit is made of: one scrim, one panel, one title bar with
      a close button, one footer. It also owns the look of the controls inside it, so a
      dialog written next month cannot arrive with the platform's own buttons on it
      (doc/12-risks.md, R-108). */
  interface Props {
    title: string;
    /** Called by the ✕, by Esc and by a click on the scrim. */
    onclose: () => void;
    /** Called by Enter, when the dialog has a single obvious confirmation. */
    onconfirm?: () => void;
    width?: string;
    /** Fixed height, for a dialog whose content must not make it jump about. */
    height?: string;
    children: Snippet;
    footer?: Snippet;
  }

  let {
    title,
    onclose,
    onconfirm,
    width = "min(440px, 90vw)",
    height,
    children,
    footer,
  }: Props = $props();

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
      return;
    }
    // Enter in a textarea is a newline, not a decision.
    if (event.key !== "Enter" || !onconfirm) return;
    if (event.target instanceof HTMLTextAreaElement) return;
    event.preventDefault();
    onconfirm();
  }
</script>

<svelte:window {onkeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onclose}></div>

<div class="dialog" role="dialog" aria-modal="true" aria-label={title} style:width style:height>
  <header>
    <h2>{title}</h2>
    <button type="button" class="close" onclick={onclose} aria-label="Close" title="Close (Esc)">
      ✕
    </button>
  </header>

  <div class="body">{@render children()}</div>

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

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--sp-4);
    flex: 0 0 auto;
    height: var(--h-toolbar);
    padding: 0 var(--sp-4) 0 var(--sp-5);
    background: var(--titlebar-bg);
    border-bottom: 1px solid var(--titlebar-border);
  }

  h2 {
    margin: 0;
    font-size: var(--fs-ui);
    font-weight: 600;
  }

  .close {
    height: auto;
    padding: 0 var(--sp-2);
    background: none;
    border: 0;
    color: var(--text-secondary);
    font: inherit;
    cursor: default;
  }

  .close:hover {
    color: var(--text-primary);
  }

  .body {
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }

  footer {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    flex: 0 0 auto;
    padding: var(--sp-4) var(--sp-5);
    border-top: 1px solid var(--divider);
  }

  /* Opt-in, by class, never by element.
     `.dialog :global(button)` compiles to `.dialog.svelte-x button`, which outweighs a
     component's own `.nav-row.svelte-y` — so it painted the category list as a column of
     boxed buttons and gave every 12px tree caret twenty pixels of padding, pushing it out
     of the panel. Three times running (doc/12-risks.md, R-125). A structural button now
     has to ask. */
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

  .dialog :global(.btn:disabled) {
    color: var(--text-secondary);
    opacity: 0.6;
  }

  .dialog :global(.btn.primary) {
    background: var(--status-ref);
    border-color: var(--status-ref);
    color: var(--c-bg-window);
    font-weight: 600;
  }

  .dialog :global(.btn.primary:hover:not(:disabled)) {
    filter: brightness(1.1);
  }

  .dialog :global(.btn.primary:disabled) {
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

  .dialog :global(input[type="checkbox"]),
  .dialog :global(input[type="radio"]) {
    accent-color: var(--status-ref);
    width: 13px;
    height: 13px;
    margin: 0;
  }
</style>
