<script lang="ts">
  /** The one empty state of the window: shown over the panels while no repository is open
      (`emptyStateVisible`). The panels themselves stay blank. Recent repositories are the
      Welcome dialog's job, so there is one list of them, not two. */
  interface Props {
    onopen: () => void;
    onclone: () => void;
    onwelcome: () => void;
  }

  let { onopen, onclone, onwelcome }: Props = $props();
</script>

<div class="layer">
  <div class="empty" role="status">
    <h2>No repository open</h2>
    <p class="lead">Open a repository, clone one, or drop a folder on the window.</p>
    <div class="buttons">
      <button type="button" class="primary" onclick={onopen}>Open…</button>
      <button type="button" onclick={onclone}>Clone…</button>
      <button type="button" onclick={onwelcome}>Welcome…</button>
    </div>
  </div>
</div>

<style>
  /* Over the workspace, but only the card takes the pointer: a repository listed in the
     Repositories panel behind the layer stays clickable. */
  .layer {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--sp-6) var(--sp-5);
    pointer-events: none;
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--sp-4);
    max-width: 400px;
    padding: var(--sp-7);
    background: var(--surface-panel);
    border: 1px solid var(--divider);
    border-radius: var(--r-sm);
    text-align: center;
    pointer-events: auto;
  }

  h2 {
    margin: 0;
    color: var(--text-primary);
    font-size: 16px;
    font-weight: 600;
  }

  .lead {
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .buttons {
    display: flex;
    gap: var(--sp-3);
    margin-top: var(--sp-3);
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

  button:hover {
    background: var(--state-hover);
  }

  button.primary {
    background: var(--status-ref);
    color: var(--fg-on-accent);
    border-color: var(--status-ref);
  }
</style>
