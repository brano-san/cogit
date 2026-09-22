<script lang="ts">
  import Checkbox from "$components/common/Checkbox.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import {
    DONT_SHOW_HINT,
    defaultAction,
    exitButtons,
    exitHeading,
    exitNote,
    focusAfterSwitch,
    showsDontShow,
    type ExitAction,
    type ExitRow,
    type ExitSource,
    type ExitVariant,
  } from "$lib/exit";
  import { untrack } from "svelte";

  /** The question, or the warning while work is queued; which one is decided in
      `$lib/exit` (doc/12-risks.md, R-151, R-168). */
  interface Props {
    source: ExitSource;
    variant: ExitVariant;
    /** One line per unfinished operation. */
    rows: readonly ExitRow[];
    waiting: boolean;
    /** The checkbox as the setting has it, so the dialog and Preferences agree. */
    dontShow: boolean;
    onanswer: (action: ExitAction, dontShowAgain: boolean) => void;
  }

  let { source, variant, rows, waiting, dontShow, onanswer }: Props = $props();

  // Seeded once: the dialog is created for one question and the box is then the user's.
  let checked = $state(untrack(() => dontShow));
  let elements = $state<Partial<Record<ExitAction, HTMLButtonElement | null>>>({});
  let focused: ExitAction | null = null;

  const buttons = $derived(exitButtons(variant, waiting));
  const note = $derived(exitNote(source, variant));

  // Re-run on a switch between the question and the warning: Cancel keeps the focus,
  // a button that went away hands it to the new default.
  $effect(() => {
    const target = focusAfterSwitch(untrack(() => focused), variant, waiting);
    elements[target]?.focus();
  });

  const answer = (action: ExitAction) => onanswer(action, checked);
</script>

<Dialog
  title="Exit"
  onclose={() => answer("cancel")}
  onconfirm={() => answer(defaultAction(variant))}
  width="min(460px, 92vw)"
>
  <div class="exit">
    <svg class="icon {variant}" viewBox="0 0 32 32" aria-hidden="true">
      {#if variant === "plain"}
        <circle cx="16" cy="16" r="15" />
        <path class="glyph" d="M12 12.6a4 4 0 1 1 5.7 3.6c-1.1.5-1.7 1.3-1.7 2.4v.6" />
        <circle class="dot" cx="16" cy="23.4" r="1.5" />
      {:else}
        <path d="M16 3.5 30 28.5H2Z" />
        <path class="glyph" d="M16 12.5v7" />
        <circle class="dot" cx="16" cy="24" r="1.5" />
      {/if}
    </svg>

    <div class="text">
      <p class="heading">{exitHeading(variant, rows.length)}</p>
      {#if note}<p class="muted">{note}</p>{/if}

      {#if variant === "busy"}
        <ul class="operations">
          {#each rows as row (row.id)}<li>{row.text}</li>{/each}
        </ul>
        <p class="muted">
          {waiting
            ? "Cogit will exit as soon as they finish."
            : "Exit Anyway stops them. A push or a rebase cut off half way can leave work behind."}
        </p>
      {/if}
    </div>
  </div>

  {#snippet footer()}
    {#if showsDontShow(variant)}
      <div class="dont-show">
        <Checkbox bind:checked label="Don't show again" />
        {#if checked}<span class="hint">{DONT_SHOW_HINT}</span>{/if}
      </div>
    {/if}
    {#each buttons as button (button.action)}
      <button
        type="button"
        class="btn"
        class:primary={button.tone === "primary"}
        class:warning={button.tone === "warning"}
        disabled={button.disabled}
        data-autofocus={button.action === defaultAction(variant) ? "" : undefined}
        bind:this={elements[button.action]}
        onfocus={() => (focused = button.action)}
        onclick={() => answer(button.action)}>{button.label}</button
      >
    {/each}
  {/snippet}
</Dialog>

<style>
  .exit {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-5);
  }

  .icon {
    flex: 0 0 32px;
    width: 32px;
    height: 32px;
  }

  .icon.plain {
    fill: var(--status-ref);
  }

  .icon.busy {
    fill: var(--status-modify);
    stroke: var(--status-modify);
    stroke-width: 2;
    stroke-linejoin: round;
  }

  .icon .glyph {
    fill: none;
    stroke: var(--surface-base);
    stroke-width: 2.4;
    stroke-linecap: round;
  }

  .icon .dot {
    fill: var(--surface-base);
    stroke: none;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-width: 0;
    padding-top: var(--sp-2);
  }

  .text p {
    margin: 0;
  }

  .heading {
    font-weight: 600;
  }

  .muted {
    color: var(--text-secondary);
  }

  .operations {
    margin: var(--sp-2) 0;
    padding: var(--sp-3) var(--sp-4) var(--sp-3) var(--sp-7);
    background: var(--surface-raised);
    border-left: 3px solid var(--status-modify);
    border-radius: var(--r-sm);
    font-variant-numeric: tabular-nums;
    user-select: text;
  }

  .dont-show {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    margin-right: auto;
  }

  .hint {
    color: var(--text-secondary);
    font-size: var(--fs-status);
  }
</style>
