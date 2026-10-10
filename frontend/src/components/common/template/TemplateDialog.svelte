<script lang="ts">
  import type { Snippet } from "svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import DialogFooter from "$components/common/template/DialogFooter.svelte";
  import type { DialogAction } from "$lib/dialog-template";

  /** The confirm-dialog template: 520 px wide, 16 px padding, 12 px between blocks,
      height by content. Put `ObjectCard`, `Callout`, `OptionRow` and `<p class="sub">` in
      `children`; Esc and the close button call `onclose`, Enter calls the primary action. */
  interface Props {
    title: string;
    onclose: () => void;
    actions: readonly DialogAction[];
    destructive?: boolean;
    /** The first thing wrong with the form, shown left of the buttons in `status.danger`. */
    status?: string | null;
    /** For a dialog whose way out is not a cancel: `Continue without git`. */
    cancelLabel?: string;
    /** Enter, when the main thing to do is not one of `actions`: Push's Fetch. */
    onenter?: () => void;
    /** The blocks share the dialog's height instead of scrolling with it: one of them
        scrolls (`flex: 1 1 auto; min-height: 0; overflow-y: auto`), the rest stay put. */
    fill?: boolean;
    children: Snippet;
  }

  let {
    title,
    onclose,
    actions,
    destructive = false,
    status = null,
    cancelLabel = "Cancel",
    onenter,
    fill = false,
    children,
  }: Props = $props();
</script>

<Dialog
  {title}
  {onclose}
  onconfirm={() => (onenter ? onenter() : actions.find((action) => action.primary && !action.disabled)?.onclick())}
  width="min(520px, 94vw)"
  surface="var(--bg-elevated)"
  inset="var(--sp-6)"
>
  <div class="stack" class:fill>{@render children()}</div>

  {#snippet footer()}
    {#if status}<span class="status" role="alert">{status}</span>{/if}
    <DialogFooter oncancel={onclose} {actions} {destructive} {cancelLabel} />
  {/snippet}
</Dialog>

<style>
  .status {
    flex: 1 1 0;
    min-width: 0;
    color: var(--status-danger);
    font-size: var(--fs-dense);
    line-height: 1.3;
    overflow-wrap: anywhere;
  }

  .stack {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
    min-width: 0;
  }

  .stack.fill {
    flex: 1 1 auto;
    min-height: 0;
  }

  /* Subtitle: `fg.secondary`, wraps. */
  .stack :global(.sub) {
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
    line-height: 1.4;
    overflow-wrap: anywhere;
  }
</style>
