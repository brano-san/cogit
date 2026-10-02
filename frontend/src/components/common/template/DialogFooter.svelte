<script lang="ts">
  import { initialFocus, type DialogAction } from "$lib/dialog-template";

  /** Footer of a template dialog: Cancel, then `actions` in order, the primary one last.
      Rendered inside `Dialog`'s `footer` snippet (its top border is the only divider). */
  interface Props {
    oncancel: () => void;
    actions: readonly DialogAction[];
    /** The primary action destroys data: it is `status.danger` and the focus starts on Cancel. */
    destructive?: boolean;
    cancelLabel?: string;
  }

  let { oncancel, actions, destructive = false, cancelLabel = "Cancel" }: Props = $props();
  const focus = $derived(initialFocus(destructive, actions));
</script>

<button type="button" class="btn" data-autofocus={focus === "cancel" || undefined} onclick={oncancel}>{cancelLabel}</button>
{#each actions as action, i (action.label)}
  <button
    type="button"
    class="btn"
    class:primary={action.primary && !destructive}
    class:danger={action.primary && destructive}
    data-autofocus={focus === i || undefined}
    title={action.tip}
    disabled={action.disabled}
    onclick={action.onclick}
  >
    {action.label}
  </button>
{/each}
