<script lang="ts">
  import { modalLayer, modals } from "$lib/modal-stack";
  import { untrack } from "svelte";
  import type { Found, FoundKind } from "$lib/ipc";

  interface Props {
    results: readonly Found[];
    busy?: boolean;
    onquery: (text: string) => void;
    onpick: (item: Found) => void;
    onclose: () => void;
  }

  let { results, busy = false, onquery, onpick, onclose }: Props = $props();

  const GROUPS: { kind: FoundKind; title: string }[] = [
    { kind: "branch", title: "Branches" },
    { kind: "tag", title: "Tags" },
    { kind: "commit", title: "Commits" },
    { kind: "file", title: "Files" },
  ];

  let query = $state("");
  let cursor = $state(0);
  let field: HTMLInputElement | undefined = $state();
  let list: HTMLDivElement | undefined = $state();

  const grouped = $derived(
    GROUPS.map((group) => ({
      ...group,
      items: results.filter((item) => item.kind === group.kind),
    })).filter((group) => group.items.length > 0),
  );
  /** The rows in the order they are drawn, which is the order the arrows walk. */
  const rows = $derived(grouped.flatMap((group) => group.items));
  const shown = $derived(!busy && query.trim() !== "" && rows.length > 0);

  /** A modal layer (11 §1): Esc closes it wherever the focus went inside it. */
  const layer = modalLayer();

  function onwindowkey(event: KeyboardEvent) {
    if (event.key !== "Escape" || !modals.isTop(layer) || event.defaultPrevented) return;
    event.preventDefault();
    onclose();
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
    }
    // The field is all there is to focus here; Tab would leave for the panels behind.
    if (event.key === "Tab") event.preventDefault();
    if (!shown) return;
    if (event.key === "ArrowDown") {
      event.preventDefault();
      cursor = Math.min(cursor + 1, rows.length - 1);
    }
    if (event.key === "ArrowUp") {
      event.preventDefault();
      cursor = Math.max(cursor - 1, 0);
    }
    if (event.key === "Enter") {
      event.preventDefault();
      const picked = rows[cursor];
      if (picked) onpick(picked);
    }
  }

  $effect(() => {
    void results;
    cursor = 0;
  });

  $effect(() => {
    void cursor;
    list?.querySelector(".row.active")?.scrollIntoView({ block: "nearest" });
  });

  // Only the query: whatever `onquery` happens to read must not start another search.
  $effect(() => {
    const text = query;
    untrack(() => onquery(text));
  });

  $effect(() => {
    field?.focus();
  });
</script>

<svelte:window onkeydown={onwindowkey} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onclose}></div>

<div class="finder" role="dialog" aria-label="Find object">
  <input
    bind:this={field}
    bind:value={query}
    {onkeydown}
    type="text"
    placeholder="Find a branch, tag, commit or file…"
    aria-label="Find an object"
  />

  {#if busy}
    <p class="note">Searching…</p>
  {:else if query.trim() === ""}
    <p class="note">Type a name, a hash, a commit message or part of a path.</p>
  {:else if grouped.length === 0}
    <p class="note">Nothing matches “{query}”.</p>
  {:else}
    <div class="list" bind:this={list}>
      {#each grouped as group (group.kind)}
        <div class="group">{group.title}</div>
        {#each group.items as item (item.kind + item.label + item.oid)}
          <div
            class="row"
            class:active={rows.indexOf(item) === cursor}
            role="button"
            tabindex="-1"
            onmouseenter={() => (cursor = rows.indexOf(item))}
            onclick={() => onpick(item)}
            onkeydown={(event) => event.key === "Enter" && onpick(item)}
          >
            <span class="label truncate">{item.label}</span>
            <span class="detail truncate">{item.detail}</span>
          </div>
        {/each}
      {/each}
    </div>
  {/if}
</div>

<style>
  .backdrop {
    position: absolute;
    inset: 0;
    z-index: 20;
    background: var(--scrim);
  }

  .finder {
    position: absolute;
    top: 12%;
    left: 50%;
    transform: translateX(-50%);
    z-index: 21;
    display: flex;
    flex-direction: column;
    width: min(620px, 82vw);
    max-height: 62vh;
    background: var(--surface-panel);
    border: 1px solid var(--field-border);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-dialog);
    overflow: hidden;
  }

  input {
    flex: 0 0 auto;
    height: 32px;
    padding: 0 var(--sp-5);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 0;
    border-bottom: 1px solid var(--divider);
    font-size: var(--fs-ui);
  }

  .list {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }

  .group {
    padding: var(--sp-3) var(--sp-5) var(--sp-2);
    background: var(--surface-raised);
    color: var(--text-secondary);
    font-size: var(--fs-header);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    height: 24px;
    padding: 0 var(--sp-5);
    font-size: var(--fs-dense);
    cursor: default;
  }

  .row.active {
    background: var(--state-selected);
  }

  .label {
    flex: 1 1 auto;
    min-width: 0;
  }

  .detail {
    flex: 0 1 auto;
    min-width: 0;
    color: var(--text-secondary);
    font-size: 10px;
  }

  .note {
    margin: 0;
    padding: var(--sp-5);
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }
</style>
