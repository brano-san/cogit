<script lang="ts">
  /** What a pane says when it has nothing to draw: centred both ways in the pane, one
      look everywhere (a diff with no change, an empty list, a window opened without a file). */
  interface Props {
    title: string;
    /** One sentence naming what to do next; a panel with nothing in it should say why. */
    hint?: string;
    action?: string;
    onaction?: () => void;
    tone?: "error" | "warn";
  }

  let { title, hint, action, onaction, tone }: Props = $props();
</script>

<div class="empty" role={tone === "error" ? "alert" : "status"}>
  <p class="title {tone ?? ''}">{title}</p>
  {#if hint}<p class="hint">{hint}</p>{/if}
  {#if action && onaction}
    <button type="button" class="btn sm" onclick={onaction}>{action}</button>
  {/if}
</div>

<style>
  .empty {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--sp-3);
    box-sizing: border-box;
    width: 100%;
    height: 100%;
    min-height: 0;
    padding: var(--sp-6) var(--sp-5);
    text-align: center;
  }

  .title {
    margin: 0;
    max-width: 60ch;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
    overflow-wrap: anywhere;
    user-select: text;
  }

  .title.error {
    color: var(--status-danger);
  }

  .title.warn {
    color: var(--status-warning);
  }

  .hint {
    margin: 0;
    max-width: 40ch;
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }
</style>
