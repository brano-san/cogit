<script lang="ts">
  import Tooltip from "$components/common/Tooltip.svelte";
  import { reasons, type Context, type Requires } from "$lib/availability";

  /** Inapplicable actions are disabled, not hidden, so buttons never move under the cursor. */
  interface Action {
    id: string;
    label: string;
    /** Lucide path data, drawn by the one <svg> below at a single size and weight. */
    icon: string;
    shortcut?: string;
    /** What the action needs; the one rule set decides whether it is offered (issue 2). */
    needs: Requires;
  }

  const ICONS = {
    pull: "M12 3v12m0 0 4-4m-4 4-4-4M5 21h14",
    push: "M12 21V9m0 0 4 4m-4-4-4 4M5 3h14",
    sync: "M21 12a9 9 0 0 1-9 9 9 9 0 0 1-8.5-6M3 12a9 9 0 0 1 9-9 9 9 0 0 1 8.5 6M21 4v5h-5M3 20v-5h5",
    stage: "M12 5v14m-7-7h14",
    unstage: "M5 12h14",
    discard: "M3 12a9 9 0 1 0 3-6.7L3 8m0-5v5h5",
    stash: "M3 8h18M3 8l2-4h14l2 4M3 8v10a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V8m-11 5h4",
    merge: "M7 18V9a4 4 0 0 1 4-4h5M7 6.5a2.5 2.5 0 1 0 0-.1M18.5 7.5a2.5 2.5 0 1 0 0-.1M7 20.5a2.5 2.5 0 1 0 0-.1",
    rebase: "M7 6.5a2.5 2.5 0 1 0 0-.1M7 20.5a2.5 2.5 0 1 0 0-.1M7 9v6M17 6.5a2.5 2.5 0 1 0 0-.1M17 9v4a4 4 0 0 1-4 4H9",
    tag: "M3 11V5a2 2 0 0 1 2-2h6l10 10-8 8L3 11Zm4-4.5a.5.5 0 1 0 0-.1",
    undo: "M3 12a9 9 0 1 1 3 6.7L3 16m0 5v-5h5",
  } as const;

  const GROUPS: Action[][] = [
    [
      { id: "pull", label: "Pull", icon: ICONS.pull, shortcut: "Ctrl+Shift+U", needs: { remote: true } },
      { id: "push", label: "Push", icon: ICONS.push, shortcut: "Ctrl+Shift+O", needs: { remote: true } },
      { id: "sync", label: "Sync", icon: ICONS.sync, shortcut: "Ctrl+Shift+S", needs: { remote: true } },
    ],
    [
      { id: "stage", label: "Stage", icon: ICONS.stage, shortcut: "Ctrl+T", needs: { selection: true } },
      { id: "unstage", label: "Unstage", icon: ICONS.unstage, shortcut: "Ctrl+Shift+T", needs: { staged: true } },
      { id: "discard", label: "Discard", icon: ICONS.discard, shortcut: "Ctrl+Z", needs: { selection: true } },
    ],
    [
      { id: "stash", label: "Stash", icon: ICONS.stash, shortcut: "Ctrl+S", needs: { changes: true } },
      { id: "merge", label: "Merge", icon: ICONS.merge, shortcut: "Ctrl+M", needs: { commit: true } },
      { id: "rebase", label: "Rebase", icon: ICONS.rebase, shortcut: "Ctrl+R", needs: { commit: true } },
      { id: "tag", label: "Tag", icon: ICONS.tag, shortcut: "Shift+F7", needs: { repository: true } },
    ],
  ];

  const UNDO: Action = {
    id: "undo",
    label: "Undo",
    icon: ICONS.undo,
    needs: { undo: true },
  };

  interface Props {
    /** Description of what Undo would reverse, for the tooltip. */
    undoable?: string;
    onundo?: () => void;
    /** Actions wired to a handler; the rest stay disabled until their module lands. */
    handlers?: Partial<Record<string, () => void>>;
    /** The state the rules read. */
    context: Context;
  }

  let { undoable, onundo, handlers = {}, context }: Props = $props();

  const blocked = $derived(
    reasons(
      Object.fromEntries([...GROUPS.flat(), UNDO].map((action) => [action.id, action.needs])),
      context,
    ),
  );

  /** Unbuilt actions stay off whatever the state says, and say so rather than lying. */
  function why(action: Action): string | undefined {
    if (action.id !== "undo" && !handlers[action.id]) return "Not built yet";
    return blocked[action.id];
  }

</script>

<div class="toolbar">
  {#each GROUPS as group, index (index)}
    {#if index > 0}
      <span class="separator" aria-hidden="true"></span>
    {/if}
    <div class="group">
      {#each group as action (action.id)}
        <Tooltip label={action.label} hint={action.shortcut ?? undefined} below>
          <button
            type="button"
            class="action"
            disabled={why(action) !== undefined}
            title={why(action)}
            aria-label="{action.label}{action.shortcut ? ` (${action.shortcut})` : ''}"
            onclick={() => handlers[action.id]?.()}
          >
            <svg class="icon" viewBox="0 0 24 24" aria-hidden="true"
              ><path d={action.icon} /></svg
            >
            <span>{action.label}</span>
          </button>
        </Tooltip>
      {/each}
    </div>
  {/each}

  <span class="separator" aria-hidden="true"></span>
  <div class="group">
    <button
      type="button"
      class="action"
      disabled={why(UNDO) !== undefined}
      title={undoable ? `Undo: ${undoable}` : why(UNDO)}
      onclick={() => onundo?.()}
    >
      <svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d={UNDO.icon} /></svg>
      <span>{UNDO.label}</span>
    </button>
  </div>

  <div class="spacer"></div>
</div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    height: var(--h-toolbar);
    flex: 0 0 var(--h-toolbar);
    padding: 0 var(--sp-5);
    background: var(--titlebar-bg);
    border-bottom: 1px solid var(--titlebar-border);
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
    flex: 0 0 auto;
    width: 15px;
    height: 15px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .spacer {
    flex: 1 1 auto;
  }
</style>
