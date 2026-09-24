<script lang="ts">
  interface Props {
    title: string;
    /** One sentence naming what to do next; a panel with nothing in it should say why. */
    hint?: string;
    action?: string;
    onaction?: () => void;
  }

  let { title, hint, action, onaction }: Props = $props();
</script>

<div class="empty">
  <p class="title">{title}</p>
  {#if hint}<p class="hint">{hint}</p>{/if}
  {#if action && onaction}
    <button type="button" onclick={onaction}>{action}</button>
  {/if}
</div>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--sp-3);
    padding: var(--sp-6) var(--sp-5);
    text-align: center;
  }

  .title {
    margin: 0;
    color: var(--text-primary);
    font-size: var(--fs-dense);
  }

  .hint {
    margin: 0;
    max-width: 40ch;
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }

  button {
    height: var(--h-input);
    padding: 0 var(--sp-4);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }
</style>
