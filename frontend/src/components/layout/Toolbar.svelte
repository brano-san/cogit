<script lang="ts">
  /** Inapplicable actions are disabled, not hidden, so buttons never move under the cursor. */
  interface Action {
    id: string;
    label: string;
    icon: string;
    shortcut?: string;
    hasMenu?: boolean;
  }

  const GROUPS: Action[][] = [
    [
      { id: "pull", label: "Pull", icon: "⭳", shortcut: "Ctrl+Shift+U", hasMenu: true },
      { id: "push", label: "Push", icon: "⭱", shortcut: "Ctrl+Shift+O", hasMenu: true },
      { id: "sync", label: "Sync", icon: "⟲", shortcut: "Ctrl+Shift+S" },
    ],
    [
      { id: "stage", label: "Stage", icon: "✓", shortcut: "Ctrl+T" },
      { id: "unstage", label: "Unstage", icon: "✗", shortcut: "Ctrl+Shift+T" },
      { id: "discard", label: "Discard", icon: "↺", shortcut: "Ctrl+Z" },
    ],
    [
      { id: "stash", label: "Stash", icon: "⚑", shortcut: "Ctrl+S", hasMenu: true },
      { id: "merge", label: "Merge", icon: "⑂", shortcut: "Ctrl+M" },
      { id: "rebase", label: "Rebase", icon: "⎇", shortcut: "Ctrl+R" },
    ],
  ];

  interface Props {
    busy?: string;
    /** Description of what Undo would reverse, or undefined when there is nothing to undo. */
    undoable?: string;
    onundo?: () => void;
  }

  let { busy, undoable, onundo }: Props = $props();

  const enabled = false;
</script>

<div class="toolbar">
  {#each GROUPS as group, index (index)}
    {#if index > 0}
      <span class="separator" aria-hidden="true"></span>
    {/if}
    <div class="group">
      {#each group as action (action.id)}
        <button
          type="button"
          class="action"
          disabled={!enabled}
          title="{action.label}{action.shortcut ? ` (${action.shortcut})` : ''}"
        >
          <span class="icon" aria-hidden="true">{action.icon}</span>
          <span>{action.label}</span>
          {#if action.hasMenu}<span class="caret" aria-hidden="true">▾</span>{/if}
        </button>
      {/each}
    </div>
  {/each}

  <span class="separator" aria-hidden="true"></span>
  <div class="group">
    <button
      type="button"
      class="action"
      disabled={!undoable}
      title={undoable ? `Undo: ${undoable}` : "Nothing to undo"}
      onclick={() => onundo?.()}
    >
      <span class="icon" aria-hidden="true">↶</span>
      <span>Undo</span>
    </button>
  </div>

  <div class="spacer"></div>

  {#if busy}
    <div class="busy" role="status">
      <span class="spinner" aria-hidden="true"></span>
      <span>{busy}</span>
    </div>
  {/if}
</div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    height: var(--h-toolbar);
    flex: 0 0 var(--h-toolbar);
    padding: 0 var(--sp-5);
    background: var(--surface-raised);
    border-bottom: 1px solid var(--divider);
  }

  .group {
    display: flex;
    align-items: center;
    gap: var(--sp-1);
  }

  .separator {
    width: 1px;
    height: 18px;
    background: var(--divider);
    margin-inline: var(--sp-3);
  }

  .action {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-3);
    height: 28px;
    padding: 0 var(--sp-5);
    border: none;
    border-radius: var(--r-md);
    background: transparent;
    color: var(--text-primary);
    font: inherit;
    font-size: var(--fs-dense);
    cursor: pointer;
    transition: background var(--t-fast) var(--ease-out);
  }

  .action:hover:not(:disabled) {
    background: var(--state-hover);
  }

  .action:active:not(:disabled) {
    background: var(--state-selected);
  }

  .action:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .icon {
    font-size: 13px;
    line-height: 1;
  }

  .caret {
    font-size: 8px;
    color: var(--text-secondary);
  }

  .spacer {
    flex: 1 1 auto;
  }

  .busy {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-3);
    font-size: var(--fs-dense);
    color: var(--text-secondary);
  }

  .spinner {
    width: 11px;
    height: 11px;
    border: 1.5px solid var(--divider);
    border-top-color: var(--status-ref);
    border-radius: 50%;
    animation: spin 700ms linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spinner {
      animation: none;
    }
  }
</style>
