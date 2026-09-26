<script lang="ts">
  import Caret from "$components/common/Caret.svelte";
  import {
    DEFAULT_LAYOUT,
    ICONS,
    groupsOf,
    hintOf,
    menuOf,
    reasonOf,
    NO_MENU_CONTEXT,
    type MenuContext,
    type MenuEntry,
    type ToolbarAction,
    type ToolbarFacts,
  } from "$lib/toolbar";

  interface Props {
    /** Description of what Undo would reverse, for the tooltip. */
    undoable?: string;
    /** Buttons and menu entries alike, keyed by id; `id:arg` entries go to `id`. */
    handlers?: Partial<Record<string, (arg?: string) => void>>;
    /** The selection and repository state the rules read (task #31). */
    facts: ToolbarFacts;
    layout?: readonly string[];
    /** Remotes and remembered choices the dropdowns are built from. */
    menus?: MenuContext;
    /** Right-click on the toolbar: offers Preferences ▸ Toolbar. */
    oncontext?: (x: number, y: number) => void;
  }

  let {
    undoable,
    handlers = {},
    facts,
    layout = DEFAULT_LAYOUT,
    menus = NO_MENU_CONTEXT,
    oncontext,
  }: Props = $props();

  const groups = $derived(groupsOf(layout));

  function handlerOf(id: string): ((arg?: string) => void) | undefined {
    const at = id.indexOf(":");
    return at < 0 ? handlers[id] : handlers[id.slice(0, at)];
  }

  /** Unbuilt actions stay off whatever the state says, and say so rather than lying. */
  function why(id: string): string | undefined {
    return reasonOf(id, facts) ?? (handlerOf(id) ? undefined : "Not built yet");
  }

  /** One state for the whole button: icon, label and caret are never out of step (#32). */
  const off = (action: ToolbarAction) => why(action.id) !== undefined;

  function tipOf(action: ToolbarAction): string {
    const reason = why(action.id);
    if (reason) return reason;
    return action.id === "undo" && undoable ? `Undo: ${undoable}` : hintOf(action, menus);
  }

  let open = $state<string | null>(null);
  /** Where the open menu hangs, measured from the button that opened it. The menu is
      rendered outside `.row`, which clips everything below itself. */
  let openAt = $state(0);

  const openMenuOf = $derived<MenuEntry[] | null>(
    open === null || open === "overflow" ? null : menuOf(open, menus),
  );

  function openMenu(id: string, event: MouseEvent) {
    if (open === id) {
      open = null;
      return;
    }
    const button = event.currentTarget as HTMLElement;
    const bar = button.closest(".toolbar");
    openAt = bar ? button.getBoundingClientRect().left - bar.getBoundingClientRect().left : 0;
    open = id;
  }
  let row: HTMLDivElement | undefined = $state();
  let crowded = $state(false);

  $effect(() => {
    const element = row;
    if (!element) return;
    // Measured with the labels showing: measured without them, the row fitted, the labels
    // came back, and the "…" that appears with them resized the row to measure again.
    const bar = element.closest(".toolbar");
    const measure = () => {
      const was = bar?.classList.contains("crowded") ?? false;
      bar?.classList.remove("crowded");
      crowded = element.scrollWidth > element.clientWidth + 1;
      if (was) bar?.classList.add("crowded");
    };
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    measure();
    return () => observer.disconnect();
  });

  function run(id: string) {
    open = null;
    const at = id.indexOf(":");
    handlerOf(id)?.(at < 0 ? undefined : id.slice(at + 1));
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="toolbar"
  class:crowded
  oncontextmenu={(event) => {
    event.preventDefault();
    oncontext?.(event.clientX, event.clientY);
  }}
>
  <div class="row" bind:this={row}>
    {#each groups as group, index (index)}
      {#if index > 0}
        <span class="separator" aria-hidden="true"></span>
      {/if}
      <div class="group">
        {#each group as action (action.id)}
          {@const disabled = off(action)}
          <!-- The tip sits on the slot: a disabled button takes no pointer events, and the
               reason it is off is what the tip then says. -->
          <div
            class="slot"
            class:disabled
            class:pressed={open === action.id}
            data-tip={tipOf(action)}
            data-tip-hint={action.shortcut}
            data-tip-below=""
          >
            <button
              type="button"
              class="quick"
              {disabled}
              aria-label="{action.label}{action.shortcut ? ` (${action.shortcut})` : ''}"
              onclick={() => run(action.id)}
            >
              <svg class="icon" viewBox="0 0 24 24" aria-hidden="true"
                ><path d={action.icon} /></svg
              >
            </button>

            {#if action.split}
              <button
                type="button"
                class="label with-menu"
                {disabled}
                aria-haspopup="menu"
                aria-expanded={open === action.id}
                title="More {action.label.toLowerCase()} actions"
                onclick={(event) => openMenu(action.id, event)}
              >
                <span class="text">{action.label}</span>
                <Caret open={open === action.id} />
              </button>
            {:else}
              <button
                type="button"
                class="label"
                {disabled}
                tabindex="-1"
                onclick={() => run(action.id)}>{action.label}</button
              >
            {/if}
          </div>
        {/each}
      </div>
    {/each}
  </div>

  <!-- Everything, not only what fell off the end: a menu whose contents shift with the
       window is harder to learn than one that never does (issue 12). -->
  {#if crowded}
    <div class="overflow">
      <button
        type="button"
        class="more"
        class:pressed={open === "overflow"}
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
          {#each groups.flat() as action (action.id)}
            <button
              type="button"
              role="menuitem"
              disabled={off(action)}
              title={tipOf(action)}
              onclick={() => run(action.id)}>{action.label}</button
            >
          {/each}
        </div>
      {/if}
    </div>
  {/if}

  <!-- Outside `.row`, which clips whatever falls below it. Positioned from the button
       that opened it (doc/12-risks.md, R-132). -->
  {#if openMenuOf}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="backdrop" onclick={() => (open = null)}></div>
    <div class="menu" role="menu" style:left="{openAt}px">
      {#each openMenuOf as entry, index (index)}
        {#if entry.kind === "separator"}
          <div class="menu-separator" role="separator"></div>
        {:else if entry.kind === "item"}
          <button
            type="button"
            role="menuitem"
            disabled={why(entry.id) !== undefined}
            title={why(entry.id) ?? entry.hint}
            onclick={() => run(entry.id)}>{entry.label}</button
          >
        {:else}
          <button
            type="button"
            role={entry.kind === "radio" ? "menuitemradio" : "menuitemcheckbox"}
            class="toggle"
            aria-checked={entry.checked}
            disabled={why(entry.id) !== undefined}
            title={why(entry.id) ?? entry.hint}
            onclick={() => run(entry.id)}
          >
            <span class="mark {entry.kind}" class:checked={entry.checked} aria-hidden="true"></span>
            {entry.label}
          </button>
        {/if}
      {/each}
    </div>
  {/if}
</div>


<style>
  .toolbar {
    position: relative;
    display: flex;
    align-items: stretch;
    height: var(--h-toolbar);
    flex: 0 0 var(--h-toolbar);
    padding: var(--sp-1) var(--sp-5);
    background: var(--surface-panel);
    border-bottom: 1px solid var(--divider);
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

  /* One highlight for the whole button: the icon used to add a second one of its own. */
  .slot:hover:not(.disabled) {
    background: var(--state-selected);
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
    height: 24px;
    padding: 0 var(--sp-3);
    border-radius: var(--r-sm);
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

  /* The line box holds the descenders: at `line-height: 1` the tail of the `g` in Tag
     hung below `.row`, which clips (#2). Icon, gap and label leave 1px spare. */
  .label {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 15px;
    font-size: var(--fs-header);
    line-height: 15px;
    white-space: nowrap;
  }

  /* Inline, not flex: the caret then sits on the middle of the lowercase letters
     whatever font the system gives, where flex centred it on the line box, above them. */
  .label.with-menu {
    display: block;
  }

  .label.with-menu :global(.caret) {
    margin-left: 2px;
  }

  .label.with-menu:hover:not(:disabled) {
    color: var(--status-ref);
  }

  /* While its menu is open the button stays down, so the menu reads as hanging from it. */
  .slot.pressed,
  .more.pressed {
    background: var(--state-selected);
    box-shadow: inset 0 0 0 1px var(--field-border);
  }

  .slot.pressed .label.with-menu {
    color: var(--status-ref);
  }

  /* Dimmed once, on the slot: dimming the parts as well dimmed them twice, and a part
     whose own state differed stood out from the rest (#32). */
  .slot.disabled {
    opacity: 0.4;
  }

  .slot.disabled button {
    pointer-events: none;
  }

  /* Narrow window: the labels go first, the icons stay recognisable (issue 12). A split
     button keeps its caret: its menu is not in the "…" one. */
  .toolbar.crowded .label:not(.with-menu),
  .toolbar.crowded .label.with-menu .text {
    display: none;
  }

  .toolbar.crowded .label.with-menu :global(.caret) {
    margin-left: 0;
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
    background: var(--state-selected);
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
    box-shadow: var(--shadow-popover);
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

  .menu-separator {
    height: 1px;
    margin: var(--sp-2) 0;
    background: var(--divider);
  }

  .menu .toggle {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding-left: var(--sp-4);
  }

  .mark {
    flex: none;
    width: 12px;
    height: 12px;
  }

  .mark.radio.checked {
    background: radial-gradient(circle, currentColor 0 3px, transparent 3.5px);
  }

  .mark.check.checked {
    background: currentColor;
    mask: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 12 12'%3E%3Cpath d='M2 6.5 5 9l5-6' fill='none' stroke='black' stroke-width='1.8' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E")
      center / contain no-repeat;
  }
</style>
