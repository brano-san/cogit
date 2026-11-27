<script lang="ts">
  import Checkbox from "$components/common/Checkbox.svelte";
  import Disclosure from "$components/common/Disclosure.svelte";
  import Icon from "$components/common/Icon.svelte";
  import VirtualList from "$components/common/VirtualList.svelte";
  import { foreign, ON_MAC, primary } from "$lib/platform";
  import type { ScanHit } from "$lib/ipc";
  import type { CheckState } from "$lib/ref-nodes";
  import { scanRange, type scanFilter, type ScanLayout, type ScanRow } from "$lib/scan-groups";
  import { scan } from "$stores/scan.svelte";
  import { uiMetricsStore } from "$stores/ui-metrics.svelte";
  import { tick, untrack } from "svelte";

  interface Props {
    rows: readonly ScanRow[];
    layout: ScanLayout;
    search: ReturnType<typeof scanFilter>;
    scope: readonly string[];
    onexpand: (group: string, expanded: boolean) => void;
  }

  let { rows, layout, search, scope, onexpand }: Props = $props();
  const id = $props.id();
  const rowId = (at: number) => id + "-result-" + at;
  const statuses = { bare: "Bare", open: "Already open" };

  let list: HTMLDivElement | undefined = $state();
  let cursor = $state<string | null>(null);
  let reveal = $state<number | null>(null);
  let descendant = $state<string | undefined>();
  let anchor: ScanHit | null = null;
  const active = $derived(Math.max(0, rows.findIndex((row) => row.id === cursor)));
  const rowHeight = $derived(uiMetricsStore.listRow + 2);

  // The focused row is named only while mounted in the virtual window.
  function syncDescendant() {
    const target = rowId(active);
    descendant = list?.contains(document.getElementById(target)) ? target : undefined;
  }

  $effect(() => {
    void rows;
    syncDescendant();
  });

  $effect(() => {
    void search;
    void layout;
    untrack(() => {
      anchor = null;
      cursor = null;
      revealRow(0);
    });
  });

  function revealRow(at: number) {
    if (reveal !== at) {
      reveal = at;
    } else {
      // Home/End must also work after scrolling away from the current row.
      reveal = null;
      const row = rows[at]?.id;
      void tick().then(() => {
        if (reveal === null && row === rows[active]?.id) reveal = active;
      });
    }
  }

  function onmousedown(event: MouseEvent) {
    if (event.button !== 0 || !(event.target instanceof Element)) return;
    const id = event.target.closest<HTMLElement>("[data-scan-row]")?.dataset.scanRow;
    if (id === undefined) return;
    event.preventDefault();
    cursor = id;
    list?.focus({ preventScroll: true });
  }

  function choose(hit: ScanHit, extend: boolean) {
    if (hit.alreadyOpen) return;
    if (extend) {
      scan.setSelection(scanRange(rows, anchor, hit), !scan.chosen.has(hit.root));
    } else {
      scan.toggle(hit.root);
      anchor = hit;
    }
  }

  function groupState(roots: readonly string[]): CheckState {
    const selected = roots.filter((root) => scan.chosen.has(root)).length;
    return selected === 0 ? "off" : selected === roots.length ? "on" : "mixed";
  }

  function toggleGroup(roots: readonly string[]): CheckState {
    scan.toggleAll(roots);
    return groupState(roots);
  }

  function onkeydown(event: KeyboardEvent) {
    const row = rows[active];
    if (!row || event.target !== list || event.altKey || event.isComposing) return;
    if (foreign(event, ON_MAC)) return;
    if (primary(event, ON_MAC)) {
      if (event.key.toLowerCase() !== "a") return;
      event.preventDefault();
      scan.setSelection(scope, true);
      return;
    }

    let next = active;
    const page = Math.max(1, Math.floor((list?.clientHeight ?? 0) / rowHeight) - 1);
    switch (event.key) {
      case "ArrowDown": next++; break;
      case "ArrowUp": next--; break;
      case "Home": next = 0; break;
      case "End": next = rows.length - 1; break;
      case "PageDown": next += page; break;
      case "PageUp": next -= page; break;
      case "ArrowRight":
        if (row.kind === "group") {
          if (row.expanded) next++;
          else onexpand(row.id, true);
        }
        break;
      case "ArrowLeft":
        if (row.kind === "group" && row.expanded) onexpand(row.id, false);
        else if (row.kind === "repo" && row.parent) next = rows.findIndex((item) => item.id === row.parent);
        break;
      case " ":
        if (row.kind === "group") scan.toggleAll(row.section.hits.map((hit) => hit.root));
        else choose(row.hit, event.shiftKey);
        break;
      case "Enter":
        if (row.kind !== "group") return;
        onexpand(row.id, !row.expanded);
        break;
      default: return;
    }
    event.preventDefault();
    next = Math.max(0, Math.min(next, rows.length - 1));
    cursor = rows[next]?.id ?? null;
    revealRow(next);
  }
</script>

{#snippet marks(value: string, path = false, root = value)}
  {#each search.parts(value, path, root) as part}
    {#if part.match}<mark>{part.text}</mark>{:else}{part.text}{/if}
  {/each}
{/snippet}

<!-- svelte-ignore a11y_no_noninteractive_tabindex (both dynamic roles are interactive) -->
<div
  class="scan-list"
  role={layout === "folders" ? "tree" : "listbox"}
  aria-label="Repositories found"
  aria-multiselectable="true"
  aria-activedescendant={descendant}
  tabindex="0"
  bind:this={list}
  {onkeydown}
  {onmousedown}
>
  <VirtualList items={rows} {rowHeight} {reveal} ondrawn={syncDescendant}>
    {#snippet row(item, at)}
      {#if item.kind === "group"}
        {@const { section, expanded } = item}
        {@const roots = section.hits.filter((hit) => !hit.alreadyOpen).map((hit) => hit.root)}
        {@const state = groupState(roots)}
        {@const status = section.kind ? statuses[section.kind] : ""}
        <!-- svelte-ignore a11y_role_has_required_aria_props (aria-checked supports mixed tree selection) -->
        <div
          class="row group"
          class:current={at === active}
          role="treeitem"
          aria-level={1}
          aria-expanded={expanded}
          aria-checked={roots.length === 0 ? undefined : state === "mixed" ? "mixed" : state === "on"}
          aria-label={[section.title, status, section.hits.length + " repositories"].filter(Boolean).join(", ")}
          id={rowId(at)}
          data-scan-row={item.id}
          style:top="{at * rowHeight}px"
          style:height="{rowHeight}px"
        >
          <button
            type="button"
            class="disclosure-button"
            tabindex="-1"
            aria-label={expanded ? "Collapse folder" : "Expand folder"}
            onclick={() => onexpand(item.id, !expanded)}
          ><Disclosure open={expanded} /></button>
          <Checkbox
            tabindex={-1}
            ariaLabel={"Select repositories in " + section.title}
            disabled={roots.length === 0}
            tri={{ state, toggle: () => toggleGroup(roots) }}
          />
          <button
            type="button"
            class="folder"
            tabindex="-1"
            data-tip={section.title}
            onclick={() => onexpand(item.id, !expanded)}
          >
            <Icon name="folder" class="scan-icon" />
            <span class="truncate">{@render marks(section.title, true, section.hits[0]?.root)}</span>
            {#if status}<span class="secondary status">{status}</span>{/if}
            <span class="secondary group-count">{section.hits.length}</span>
          </button>
        </div>
      {:else}
        {@const { hit, parent } = item}
        {@const selected = scan.chosen.has(hit.root)}
        {@const status = hit.alreadyOpen ? statuses.open : hit.bare ? statuses.bare : ""}
        <div
          class="row repository"
          class:nested={parent !== null}
          class:current={at === active}
          class:checked={selected}
          role={parent === null ? "option" : "treeitem"}
          aria-level={parent === null ? undefined : 2}
          aria-checked={parent === null || hit.alreadyOpen ? undefined : selected}
          aria-selected={parent === null ? selected : undefined}
          aria-disabled={hit.alreadyOpen || undefined}
          aria-label={[hit.name, parent === null ? hit.root : "", status].filter(Boolean).join(", ")}
          id={rowId(at)}
          data-scan-row={item.id}
          data-tip={hit.root}
          style:top="{at * rowHeight}px"
          style:height="{rowHeight}px"
        >
          <Checkbox wide tabindex={-1} checked={selected} disabled={hit.alreadyOpen} onclick={(event) => choose(hit, event.shiftKey)}>
            <Icon name="repository" class="scan-icon" />
            <span class="name truncate">{@render marks(hit.name, false, hit.root)}</span>
            {#if parent === null}
              <span class="secondary path truncate">{@render marks(hit.root, true)}</span>
              {#if status}<span class="secondary status">{status}</span>{/if}
            {/if}
          </Checkbox>
        </div>
      {/if}
    {/snippet}
  </VirtualList>
</div>

<style>
  .scan-list {
    display: flex;
    flex-direction: column;
    flex: 1 1 auto;
    min-height: 0;
    background: var(--surface-base);
  }

  .scan-list[aria-activedescendant]:focus-visible {
    outline: none;
  }

  .scan-list:focus-visible .current {
    outline: 1px solid var(--state-focus-ring);
    outline-offset: -1px;
  }

  .row {
    position: absolute;
    inset-inline: 0;
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    padding-inline: var(--sp-5);
    font-size: var(--fs-dense);
    white-space: nowrap;
  }

  .group {
    background: var(--surface-panel);
    font-weight: 600;
  }

  .disclosure-button,
  .folder {
    display: flex;
    align-items: center;
    height: 100%;
    gap: var(--sp-3);
    cursor: default;
  }

  .folder {
    flex: 1 1 auto;
    min-width: 0;
    text-align: start;
  }

  .folder .truncate {
    flex: 1 1 auto;
  }

  .group-count {
    min-width: 3ch;
    margin-inline-start: auto;
    text-align: end;
    font-variant-numeric: tabular-nums;
  }

  .row :global(.scan-icon) {
    width: var(--panel-icon);
    height: var(--panel-icon);
    align-self: center;
    color: var(--text-secondary);
  }

  .repository :global(.checkbox) {
    align-self: stretch;
    align-items: center;
  }

  .repository :global(.box) {
    margin-top: 0;
  }

  .repository.nested {
    padding-inline-start: calc(var(--sp-5) + var(--disclosure-glyph) + 2 * var(--sp-3));
  }

  .row.checked {
    background: var(--state-selected);
  }

  .row:not(.checked):hover {
    background: var(--state-hover);
  }

  .name {
    flex: 0 1 auto;
    min-width: 0;
  }

  .name:has(+ .path) {
    max-width: 55%;
  }

  .secondary {
    color: var(--text-secondary);
    font-size: var(--fs-header);
    font-weight: 400;
  }

  .path {
    flex: 1 1 0;
  }

  .status {
    flex: none;
  }

  mark {
    color: var(--state-pressed-text);
    background: var(--state-pressed);
    font-weight: 600;
  }
</style>
