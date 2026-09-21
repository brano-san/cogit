<script lang="ts">
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
  let field: HTMLInputElement | undefined = $state();

  const grouped = $derived(
    GROUPS.map((group) => ({
      ...group,
      items: results.filter((item) => item.kind === group.kind),
    })).filter((group) => group.items.length > 0),
  );

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
    }
  }

  $effect(() => {
    onquery(query);
  });

  $effect(() => {
    field?.focus();
  });
</script>

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
    <div class="list">
      {#each grouped as group (group.kind)}
        <div class="group">{group.title}</div>
        {#each group.items as item (item.kind + item.label + item.oid)}
          <div
            class="row"
            role="button"
            tabindex="-1"
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
    font-size: var(--fs-body, 13px);
  }

  .list {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
  }

  .group {
    padding: var(--sp-3) var(--sp-5) var(--sp-2, 3px);
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

  .row:hover {
    background: var(--state-hover);
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
