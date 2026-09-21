<script lang="ts">
  import Tooltip from "$components/common/Tooltip.svelte";
  import { reasons, type Context, type Requires } from "$lib/availability";

  /** One entry of a split button's dropdown. */
  interface Choice {
    id: string;
    label: string;
    hint: string;
    needs: Requires;
  }

  /** Inapplicable actions are disabled, not hidden, so buttons never move under the cursor. */
  interface Action {
    id: string;
    label: string;
    /** Lucide path data, drawn by the one <svg> below at a single size and weight. */
    icon: string;
    hint: string;
    shortcut?: string;
    /** What the action needs; the one rule set decides whether it is offered (issue 2). */
    needs: Requires;
    /** Absent means a plain button: no caret, no second click target (issue 9). */
    menu?: Choice[];
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
    rebase:
      "M7 6.5a2.5 2.5 0 1 0 0-.1M7 20.5a2.5 2.5 0 1 0 0-.1M7 9v6M17 6.5a2.5 2.5 0 1 0 0-.1M17 9v4a4 4 0 0 1-4 4H9",
    tag: "M3 11V5a2 2 0 0 1 2-2h6l10 10-8 8L3 11Zm4-4.5a.5.5 0 1 0 0-.1",
    undo: "M3 12a9 9 0 1 1 3 6.7L3 16m0 5v-5h5",
    more: "M5 12h.01M12 12h.01M19 12h.01",
  } as const;

  const GROUPS: Action[][] = [
    [
      {
        id: "pull",
        label: "Pull",
        icon: ICONS.pull,
        hint: "Bring the remote's commits down",
        shortcut: "Ctrl+Shift+U",
        needs: { remote: true },
        menu: [
          { id: "fetch", label: "Fetch", hint: "Update the remote refs, change nothing here", needs: { remote: true } },
          { id: "pull", label: "Pull", hint: "Fetch, then merge", needs: { remote: true } },
          { id: "fetch-all", label: "Fetch All", hint: "Every remote of every open repository", needs: { remote: true } },
        ],
      },
      {
        id: "push",
        label: "Push",
        icon: ICONS.push,
        hint: "Send your commits to the remote",
        shortcut: "Ctrl+Shift+O",
        needs: { remote: true },
      },
      {
        id: "sync",
        label: "Sync",
        icon: ICONS.sync,
        hint: "Fetch every remote",
        shortcut: "Ctrl+Shift+S",
        needs: { remote: true },
      },
    ],
    [
      { id: "stage", label: "Stage", icon: ICONS.stage, hint: "Move the ticked files into the index", shortcut: "Ctrl+T", needs: { selection: true } },
      { id: "unstage", label: "Unstage", icon: ICONS.unstage, hint: "Take them back out of the index", shortcut: "Ctrl+Shift+T", needs: { staged: true } },
      { id: "discard", label: "Discard", icon: ICONS.discard, hint: "Throw the changes away", shortcut: "Ctrl+Z", needs: { selection: true } },
    ],
    [
      {
        id: "stash",
        label: "Stash",
        icon: ICONS.stash,
        hint: "Put the working tree aside",
        shortcut: "Ctrl+S",
        needs: { changes: true },
        menu: [
          { id: "stash", label: "Stash All", hint: "Everything in the working tree", needs: { changes: true } },
          { id: "stash-selection", label: "Stash Selection", hint: "Only the ticked files", needs: { selection: true } },
        ],
      },
      { id: "merge", label: "Merge", icon: ICONS.merge, hint: "Merge the selected commit into HEAD", shortcut: "Ctrl+M", needs: { commit: true } },
      {
        id: "rebase",
        label: "Rebase",
        icon: ICONS.rebase,
        hint: "Replay HEAD on the selected commit",
        shortcut: "Ctrl+R",
        needs: { commit: true },
        menu: [
          { id: "rebase", label: "Rebase", hint: "Replay HEAD on the selected commit", needs: { commit: true } },
          { id: "rebase-i", label: "Interactive Rebase…", hint: "Edit the list of commits first", needs: { commit: true } },
        ],
      },
      { id: "tag", label: "Tag", icon: ICONS.tag, hint: "Tag the current commit", shortcut: "Shift+F7", needs: { repository: true } },
    ],
  ];

  const UNDO: Action = {
    id: "undo",
    label: "Undo",
    icon: ICONS.undo,
    hint: "Reverse the last operation",
    needs: { repository: true, undo: true },
  };

  const EVERY: Action[] = [...GROUPS.flat(), UNDO];

  interface Props {
    /** Description of what Undo would reverse, for the tooltip. */
    undoable?: string;
    onundo?: () => void;
    /** Quick actions and menu entries alike, keyed by id. */
    handlers?: Partial<Record<string, () => void>>;
    context: Context;
  }

  let { undoable, onundo, handlers = {}, context }: Props = $props();

  const blocked = $derived(
    reasons(
      Object.fromEntries(
        EVERY.flatMap((action) => [
          [action.id, action.needs] as const,
          ...(action.menu ?? []).map((choice) => [`menu:${choice.id}`, choice.needs] as const),
        ]),
      ),
      context,
    ),
  );

  /** Unbuilt actions stay off whatever the state says, and say so rather than lying. */
  function why(id: string, key: string): string | undefined {
    if (id !== "undo" && !handlers[id]) return "Not built yet";
    return blocked[key];
  }

  const off = (action: Action) => why(action.id, action.id) !== undefined;

  /** The caret is live while any one entry is, even when the quick action is not. */
  function menuOff(action: Action): boolean {
    return (action.menu ?? []).every((choice) => why(choice.id, `menu:${choice.id}`) !== undefined);
  }

  let open = $state<string | null>(null);
  let row: HTMLDivElement | undefined = $state();
  let crowded = $state(false);

  $effect(() => {
    const element = row;
    if (!element) return;
    const measure = () => {
      crowded = element.scrollWidth > element.clientWidth + 1;
    };
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    measure();
    return () => observer.disconnect();
  });

  function run(id: string) {
    open = null;
    handlers[id]?.();
  }

  function undoNow() {
    open = null;
    onundo?.();
  }
</script>

<div class="toolbar" class:crowded>
  <div class="row" bind:this={row}>
    {#each GROUPS as group, index (index)}
      {#if index > 0}
        <span class="separator" aria-hidden="true"></span>
      {/if}
      <div class="group">
        {#each group as action (action.id)}
          <div class="slot" class:disabled={off(action)}>
            <Tooltip label={action.hint} hint={action.shortcut ?? undefined} below>
              <button
                type="button"
                class="quick"
                disabled={off(action)}
                aria-label="{action.label}{action.shortcut ? ` (${action.shortcut})` : ''}"
                onclick={() => run(action.id)}
              >
                <svg class="icon" viewBox="0 0 24 24" aria-hidden="true"
                  ><path d={action.icon} /></svg
                >
              </button>
            </Tooltip>

            {#if action.menu}
              <button
                type="button"
                class="label with-menu"
                disabled={menuOff(action)}
                aria-haspopup="menu"
                aria-expanded={open === action.id}
                title="More {action.label.toLowerCase()} actions"
                onclick={() => (open = open === action.id ? null : action.id)}
              >
                <span>{action.label}</span>
                <span class="caret" aria-hidden="true">▾</span>
              </button>
            {:else}
              <button
                type="button"
                class="label"
                disabled={off(action)}
                tabindex="-1"
                onclick={() => run(action.id)}>{action.label}</button
              >
            {/if}

            {#if open === action.id && action.menu}
              <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
              <div class="backdrop" onclick={() => (open = null)}></div>
              <div class="menu" role="menu">
                {#each action.menu as choice (choice.id)}
                  <button
                    type="button"
                    role="menuitem"
                    disabled={why(choice.id, `menu:${choice.id}`) !== undefined}
                    title={why(choice.id, `menu:${choice.id}`) ?? choice.hint}
                    onclick={() => run(choice.id)}>{choice.label}</button
                  >
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/each}

    <span class="separator" aria-hidden="true"></span>
    <div class="group">
      <div class="slot" class:disabled={off(UNDO)}>
        <Tooltip label={undoable ? `Undo: ${undoable}` : UNDO.hint} below>
          <button
            type="button"
            class="quick"
            disabled={off(UNDO)}
            aria-label={UNDO.label}
            onclick={undoNow}
          >
            <svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d={UNDO.icon} /></svg>
          </button>
        </Tooltip>
        <button type="button" class="label" disabled={off(UNDO)} tabindex="-1" onclick={undoNow}
          >{UNDO.label}</button
        >
      </div>
    </div>
  </div>

  <!-- Everything, not only what fell off the end: a menu whose contents shift with the
       window is harder to learn than one that never does (issue 12). -->
  {#if crowded}
    <div class="overflow">
      <button
        type="button"
        class="more"
        aria-haspopup="menu"
        aria-expanded={open === "overflow"}
        title="More actions"
        onclick={() => (open = open === "overflow" ? null : "overflow")}
      >
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true"><path d={ICONS.more} /></svg>
      </button>
      {#if open === "overflow"}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div class="backdrop" onclick={() => (open = null)}></div>
        <div class="menu right" role="menu">
          {#each EVERY as action (action.id)}
            <button
              type="button"
              role="menuitem"
              disabled={off(action)}
              title={why(action.id, action.id) ?? action.hint}
              onclick={() => (action.id === "undo" ? undoNow() : run(action.id))}
              >{action.label}</button
            >
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .toolbar {
    display: flex;
    align-items: stretch;
    height: var(--h-toolbar);
    flex: 0 0 var(--h-toolbar);
    padding: 0 var(--sp-3);
    background: var(--titlebar-bg);
    border-bottom: 1px solid var(--titlebar-border);
  }

  .row {
    display: flex;
    align-items: stretch;
    gap: var(--sp-2);
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
  }

  .group {
    display: flex;
    align-items: stretch;
  }

  .separator {
    align-self: center;
    width: 1px;
    height: 26px;
    background: var(--divider);
    margin-inline: var(--sp-3);
  }

  /* Icon above, label and caret below: the two click targets of a split button, as in
     SmartGit. A plain action keeps the same shape so the row stays even. */
  .slot {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1px;
    padding: 0 var(--sp-2);
    border-radius: var(--r-md);
  }

  .slot:hover:not(.disabled) {
    background: var(--state-hover);
  }

  .quick,
  .label {
    padding: 0;
    background: none;
    border: 0;
    color: var(--text-primary);
    font: inherit;
    cursor: default;
  }

  .quick {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 26px;
    padding: 0 var(--sp-3);
    border-radius: var(--r-sm);
  }

  .quick:hover:not(:disabled) {
    background: var(--state-selected);
  }

  .icon {
    width: 22px;
    height: 22px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .label {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 13px;
    font-size: var(--fs-header);
    line-height: 1;
    white-space: nowrap;
  }

  .label.with-menu:hover:not(:disabled) {
    color: var(--status-ref);
  }

  .caret {
    font-size: 8px;
    color: var(--text-secondary);
  }

  .slot.disabled,
  .quick:disabled,
  .label:disabled {
    opacity: 0.4;
  }

  .label:disabled .caret {
    color: inherit;
  }

  /* Narrow window: the labels go first, the icons stay recognisable (issue 12). */
  .toolbar.crowded .label {
    display: none;
  }

  .overflow {
    position: relative;
    display: flex;
    align-items: center;
    flex: 0 0 auto;
    padding-left: var(--sp-2);
  }

  .more {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 28px;
    padding: 0 var(--sp-3);
    background: none;
    border: 0;
    border-radius: var(--r-md);
    color: var(--text-primary);
    cursor: default;
  }

  .more:hover {
    background: var(--state-hover);
  }

  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 30;
  }

  .menu {
    position: absolute;
    z-index: 31;
    top: calc(100% + 2px);
    left: 0;
    min-width: 180px;
    padding: var(--sp-2) 0;
    background: var(--surface-raised);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    box-shadow: 0 8px 24px rgb(0 0 0 / 45%);
  }

  .menu.right {
    left: auto;
    right: 0;
  }

  .menu button {
    display: block;
    width: 100%;
    padding: var(--sp-2) var(--sp-5);
    background: none;
    border: 0;
    color: var(--text-primary);
    font: inherit;
    font-size: var(--fs-dense);
    text-align: left;
    cursor: default;
  }

  .menu button:hover:not(:disabled) {
    background: var(--state-hover);
  }

  .menu button:disabled {
    opacity: 0.4;
  }
</style>
