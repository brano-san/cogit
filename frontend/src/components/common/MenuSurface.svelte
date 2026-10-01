<script lang="ts">
  import { tick } from "svelte";
  import { modalLayer, modals } from "$lib/modal-stack";
  import {
    firstEnabled,
    hoverRow,
    navKey,
    openFocused,
    rowsAt,
    typeaheadTo,
    TypedWord,
    type MenuRow,
    type Nav,
  } from "$lib/menu-nav";
  import { placeAtPoint, placeDropdown, placeSubmenu, type Box } from "$lib/popup-place";

  /** The page's own menu: one list of rows, the submenus it opens, and the keys and pointer
      that drive them. The context menu and every drop-down of the menu bar are this. */
  interface Props {
    rows: readonly MenuRow[];
    /** A point (context menu) or the box a drop-down hangs from (a title of the bar). */
    at: { x: number; y: number } | { box: Box };
    /** Left and right arrows at the top level go to the neighbouring drop-down. */
    bar?: boolean;
    /** Opened from the keyboard: the first row is already focused. */
    focusFirst?: boolean;
    label?: string;
    onpick: (row: MenuRow) => void;
    onclose: () => void;
    onbar?: (step: -1 | 1) => void;
  }

  let { rows, at, bar = false, focusFirst = false, label, onpick, onclose, onbar }: Props = $props();

  /** Esc and every other key are this menu's while it is open and nothing is above it. */
  const layer = modalLayer();

  // The menu is keyed by what opens it: the first row is chosen once, at the start.
  // svelte-ignore state_referenced_locally
  let nav = $state.raw<Nav>({ path: [focusFirst ? firstEnabled(rows) : -1] });
  let panels: HTMLElement[] = $state.raw([]);
  let places = $state.raw<Record<number, { left: number; top: number; maxHeight: number }>>({});

  const OPEN_MS = 150;
  const CLOSE_MS = 250;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const typed = new TypedWord();

  const depthOpen = $derived(nav.path.length);

  function viewport() {
    return { width: window.innerWidth, height: window.innerHeight };
  }

  function rowBox(depth: number): Box | null {
    const element = panels[depth - 1]?.querySelector<HTMLElement>(`[data-index="${nav.path[depth - 1]}"]`);
    if (!element) return null;
    const box = element.getBoundingClientRect();
    return { left: box.left, top: box.top, right: box.right, bottom: box.bottom };
  }

  /** After every change of what is open: measure what rendered, then put it somewhere that
      is inside the window. Hidden until then, so a menu never flashes where it was not meant. */
  $effect(() => {
    const open = depthOpen;
    void rows;
    void tick().then(() => {
      const next: Record<number, { left: number; top: number; maxHeight: number }> = {};
      for (let depth = 0; depth < open; depth++) {
        const panel = panels[depth];
        if (!panel) continue;
        const size = { width: panel.offsetWidth, height: panel.scrollHeight + 2 };
        if (depth === 0) {
          next[0] = "box" in at ? placeDropdown(at.box, size, viewport()) : placeAtPoint(at, size, viewport());
        } else {
          const anchor = rowBox(depth);
          if (anchor) next[depth] = placeSubmenu(anchor, size, viewport());
        }
      }
      places = next;
    });
  });

  $effect(() => {
    const index = nav.path[nav.path.length - 1];
    const panel = panels[nav.path.length - 1];
    panel?.querySelector<HTMLElement>(`[data-index="${index}"]`)?.scrollIntoView({ block: "nearest" });
  });

  function setNav(next: Nav) {
    clearTimeout(timer);
    nav = next;
  }

  function onkeydown(event: KeyboardEvent) {
    if (!modals.isTop(layer)) return;
    const depth = nav.path.length - 1;
    const here = rowsAt(rows, nav.path, depth);
    if (event.key.length === 1 && event.key !== " " && !event.ctrlKey && !event.metaKey && !event.altKey) {
      event.preventDefault();
      event.stopPropagation();
      const to = typeaheadTo(here, nav.path[depth] ?? -1, typed.add(event.key, performance.now()));
      if (to !== null) setNav({ path: [...nav.path.slice(0, -1), to] });
      return;
    }
    const result = navKey(rows, nav, event.key, bar);
    if (result.kind === "none") return;
    event.preventDefault();
    event.stopPropagation();
    if (result.kind === "state") setNav(result.nav);
    else if (result.kind === "activate") onpick(result.row);
    else if (result.kind === "close") onclose();
    else onbar?.(result.step);
  }

  function enter(depth: number, index: number, row: MenuRow) {
    clearTimeout(timer);
    if (!row.enabled) return;
    const opened = nav.path.length > depth + 1;
    const apply = () => {
      nav = hoverRow(nav, depth, index);
      if (row.children.length > 0) timer = setTimeout(() => (nav = openFocused(rows, nav)), OPEN_MS);
    };
    // Moving from a row with an open submenu toward it: wait before closing what is open,
    // so the pointer can cross the corner of the row below.
    if (opened && nav.path[depth] !== index) timer = setTimeout(apply, CLOSE_MS);
    else apply();
  }

  function click(depth: number, index: number, row: MenuRow) {
    if (!row.enabled) return;
    if (row.children.length > 0) {
      clearTimeout(timer);
      nav = openFocused(rows, hoverRow(nav, depth, index));
      return;
    }
    onpick(row);
  }

  /** A menu cannot outlive the window that is not looked at, or one that changed size. */
  function leave() {
    onclose();
  }
</script>

<svelte:window {onkeydown} onblur={leave} onresize={leave} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div
  class="backdrop"
  onpointerdown={(event) => {
    event.preventDefault();
    onclose();
  }}
  oncontextmenu={(event) => {
    event.preventDefault();
    onclose();
  }}
></div>

{#snippet panel(list: readonly MenuRow[], depth: number)}
  {@const place = places[depth]}
  {@const ticks = list.some((row) => row.checked !== null)}
  <div
    class="menu"
    role="menu"
    tabindex="-1"
    aria-label={depth === 0 ? label : undefined}
    bind:this={panels[depth]}
    style:left="{place?.left ?? 0}px"
    style:top="{place?.top ?? 0}px"
    style:max-height={place ? `${place.maxHeight}px` : undefined}
    style:visibility={place ? "visible" : "hidden"}
    onpointerenter={() => clearTimeout(timer)}
  >
    {#each list as row, index (index)}
      {#if row.separator}
        <div class="sep" role="separator"></div>
      {:else}
        {@const focused = nav.path[depth] === index}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          class="row"
          class:focus={focused}
          class:off={!row.enabled}
          role={row.checked === null ? "menuitem" : "menuitemcheckbox"}
          aria-checked={row.checked ?? undefined}
          aria-disabled={!row.enabled}
          aria-haspopup={row.children.length > 0 ? "menu" : undefined}
          aria-expanded={row.children.length > 0 ? nav.path.length > depth + 1 && nav.path[depth] === index : undefined}
          tabindex="-1"
          data-index={index}
          onpointerenter={() => enter(depth, index, row)}
          onpointerdown={(event) => event.preventDefault()}
          onclick={() => click(depth, index, row)}
        >
          {#if ticks}<span class="tick" aria-hidden="true">{#if row.checked}<svg viewBox="0 0 10 10"><path d="M1.8 5.4 4 7.6 8.2 2.6" /></svg>{/if}</span>{/if}
          <span class="text">{row.label}</span>
          {#if row.shortcut}<span class="keys">{row.shortcut}</span>{/if}
          {#if row.children.length > 0}
            <svg class="arrow" viewBox="0 0 10 10" aria-hidden="true"><path d="M3.5 2 6.5 5 3.5 8" /></svg>
          {/if}
        </div>
        {#if nav.path.length > depth + 1 && nav.path[depth] === index && row.children.length > 0}
          {@render panel(row.children, depth + 1)}
        {/if}
      {/if}
    {/each}
  </div>
{/snippet}

{@render panel(rows, 0)}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 1000;
  }

  .menu {
    position: fixed;
    z-index: 1002;
    display: flex;
    flex-direction: column;
    min-width: 180px;
    max-width: calc(100vw - 8px);
    padding: var(--sp-2) 0;
    overflow-y: auto;
    background: var(--bg-elevated);
    color: var(--fg-primary);
    border: 1px solid var(--border-strong);
    border-radius: var(--r-sm);
    box-shadow: var(--shadow-popover);
    font-family: var(--font-ui);
    font-size: var(--fs-ui);
    line-height: var(--lh-ui);
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    flex: none;
    min-height: var(--h-row);
    padding: 0 var(--sp-5);
    color: var(--fg-primary);
    cursor: default;
    white-space: nowrap;
  }

  /* Not --bg-hover: on the elevated surface it is the same color in the dark themes. */
  .row.focus {
    background: var(--bg-selected);
  }

  .row.off {
    color: var(--fg-disabled);
  }

  .text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .keys {
    margin-left: var(--sp-6);
    color: var(--fg-muted);
  }

  .row.off .keys {
    color: var(--fg-disabled);
  }

  .tick {
    display: inline-flex;
    flex: none;
    width: var(--menu-caret);
  }

  .tick svg,
  .arrow {
    width: var(--menu-caret);
    height: var(--menu-caret);
    flex: none;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .arrow {
    margin-right: calc(-1 * var(--sp-2));
  }

  .sep {
    flex: none;
    height: 1px;
    margin: var(--sp-2) 0;
    background: var(--border);
  }
</style>
