<script lang="ts">
  interface Props {
    recent: readonly string[];
    onopen: () => void;
    onscan: () => void;
    onpick: (root: string) => void;
    onforget: (root: string) => void;
  }

  let { recent, onopen, onscan, onpick, onforget }: Props = $props();

  function name(path: string): string {
    return path.replace(/[\\/]+$/, "").split(/[\\/]/).at(-1) ?? path;
  }
</script>

<div class="start">
  <h2>Cogit</h2>
  <p class="lead">Open a repository, scan a folder for several, or drop a folder on the window.</p>

  <div class="buttons">
    <button type="button" class="primary" onclick={onopen}>Open Repository…</button>
    <button type="button" onclick={onscan}>Scan Folder…</button>
  </div>

  {#if recent.length > 0}
    <h3>Recent</h3>
    <ul class="recent">
      {#each recent as path (path)}
        <li>
          <button type="button" class="entry" title={path} onclick={() => onpick(path)}>
            <span class="name">{name(path)}</span>
            <span class="path truncate">{path}</span>
          </button>
          <button
            type="button"
            class="forget"
            title="Forget this one — the folder stays on disk"
            onclick={() => onforget(path)}>✕</button
          >
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .start {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding: var(--sp-6, 16px) var(--sp-5);
    overflow: auto;
  }

  h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
  }

  h3 {
    margin: var(--sp-4) 0 0;
    color: var(--text-secondary);
    font-size: var(--fs-header);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .lead {
    margin: 0;
    max-width: 52ch;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .buttons {
    display: flex;
    gap: var(--sp-3);
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

  button.primary {
    background: var(--status-ref);
    color: var(--c-bg-window);
    border-color: var(--status-ref);
  }

  .recent {
    display: flex;
    flex-direction: column;
    gap: 1px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .recent li {
    display: flex;
    align-items: center;
    gap: var(--sp-2, 3px);
  }

  .entry {
    display: flex;
    flex: 1 1 auto;
    min-width: 0;
    align-items: baseline;
    gap: var(--sp-3);
    height: var(--h-row-dense);
    padding: 0 var(--sp-3);
    background: none;
    border: 0;
    text-align: left;
  }

  .entry:hover {
    background: var(--state-hover);
  }

  .name {
    flex: 0 0 auto;
  }

  .path {
    min-width: 0;
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }

  .forget {
    flex: 0 0 auto;
    height: var(--h-row-dense);
    padding: 0 var(--sp-3);
    background: none;
    border: 0;
    color: var(--text-secondary);
  }

  .forget:hover {
    color: var(--status-delete);
  }
</style>
