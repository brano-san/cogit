<script lang="ts">
  import type { Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";

  /** The app's push button: the one look `Find`, `Stage lines`, dialog footers and the
      metadata rows share. `sm` fits a panel bar or a metadata row (`--h-button-sm`), `md` a
      dialog footer (`--h-button`). `title` / `data-tip` are answered by the shared tooltip. */
  interface Props extends Omit<HTMLButtonAttributes, "class" | "children"> {
    variant?: "default" | "primary" | "danger";
    size?: "sm" | "md";
    class?: string;
    children?: Snippet;
  }

  let { variant = "default", size = "sm", class: cls = "", type = "button", children, ...rest }: Props = $props();
</script>

<button {type} class="cogit-btn {size} {variant} {cls}" {...rest}>{@render children?.()}</button>

<style>
  .cogit-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--sp-2);
    flex: 0 0 auto;
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font: inherit;
    font-size: var(--fs-dense);
    white-space: nowrap;
    cursor: default;
  }

  .sm {
    height: var(--h-button-sm);
  }

  .md {
    height: var(--h-button);
    padding: 0 var(--sp-5);
  }

  .cogit-btn:hover:not(:disabled) {
    border-color: var(--state-focus-ring);
  }

  .cogit-btn:focus-visible {
    outline: 1px solid var(--state-focus-ring);
    outline-offset: 1px;
  }

  /* 06 §6: a control that cannot act says so, and does not light up under the pointer. */
  .cogit-btn:disabled {
    color: var(--text-secondary);
    opacity: 0.6;
  }

  .primary,
  .danger {
    background: var(--status-ref);
    border-color: var(--status-ref);
    color: var(--fg-on-accent);
    font-weight: 600;
  }

  .danger {
    background: var(--status-danger);
    border-color: var(--status-danger);
  }

  .primary:hover:not(:disabled),
  .danger:hover:not(:disabled) {
    filter: brightness(1.1);
  }

  .primary:disabled,
  .danger:disabled {
    background: var(--surface-input);
    border-color: var(--field-border);
    color: var(--text-secondary);
    font-weight: 400;
  }
</style>
