<script lang="ts">
  import Icon from "$components/common/Icon.svelte";
  import RepositoryMarks from "$components/repo-tree/RepositoryMarks.svelte";
  import { pointerDrag } from "$lib/pointer-drag";
  import { listedName } from "$lib/repo-list";
  import { freshOverview, rowSync, syncTooltip } from "$lib/repo-sync";
  import { repoList } from "$stores/repo-list.svelte";
  import { repoPulse } from "$stores/repo-pulse.svelte";
  import { repository } from "$stores/repository.svelte";
  import { session } from "$stores/session.svelte";

  let { active, onselect, onclose, oncontext }: {
    active: string | null;
    onselect: (root: string) => void;
    onclose: (root: string) => void;
    oncontext: (root: string, x: number, y: number) => void;
  } = $props();

  let strip: HTMLDivElement | undefined = $state();
  let over: string | null = $state(null);
  const byRoot = $derived(new Map(repository.openRepos.map((item) => [item.root, item])));
  const drag = {
    onover: (target: string | null) => (over = target),
    ondrop: (root: string, target: string) => session.moveTab(root, target),
  };

  $effect(() => {
    if (active) strip?.querySelector<HTMLElement>(`[data-drop="${CSS.escape(active)}"]`)
      ?.scrollIntoView({ block: "nearest", inline: "nearest" });
  });

  function middleClick(event: MouseEvent, root: string) {
    if (event.button !== 1) return;
    event.preventDefault();
    onclose(root);
  }
</script>

<div class="tabs" role="tablist" aria-label="Repositories" bind:this={strip} use:pointerDrag={drag}>
  {#each session.repositories as root (root)}
    {@const entry = byRoot.get(root)}
    {@const name = listedName(repoList.list, root, entry ?? null)}
    {@const sync = rowSync({
      overview: entry ? freshOverview(entry, repository.current, repository.statusKnown ? null : entry) : null,
      owned: !!entry && repository.current?.repo === entry.repo,
      pulse: repoPulse.pulses.get(root),
      fetchFailed: repoPulse.unknown.has(root),
      remoteAhead: repoPulse.remoteAhead.has(root),
    })}
    {@const tip = syncTooltip(sync)}
    <div class="tab" class:active={root === active} class:over={root === over} class:closed={!entry} data-drop={root}
      data-tip={tip || root} data-tip-context={tip ? root : undefined}
      role="presentation" oncontextmenu={(event) => {
        event.preventDefault();
        oncontext(root, event.clientX, event.clientY);
      }}>
      <button class="select" type="button" role="tab" aria-selected={active === root}
        data-drag={root} onclick={() => onselect(root)}
        onmousedown={(event) => { if (event.button === 1) event.preventDefault(); }}
        onauxclick={(event) => middleClick(event, root)}>
        <RepositoryMarks {sync} tooltip={false} />
        <span class="truncate">{name}</span>
      </button>
      <button class="close" type="button" aria-label="Close {name}"
        onclick={() => onclose(root)} onauxclick={(event) => middleClick(event, root)}>
        <Icon name="close" />
      </button>
    </div>
  {/each}
</div>

<style>
  .tabs {
    display: flex;
    flex: none;
    min-width: 0;
    overflow-x: auto;
    scrollbar-width: thin;
    background: var(--surface-panel);
    border-bottom: 1px solid var(--divider);
  }

  .tab,
  button {
    display: flex;
    align-items: center;
  }

  .tab {
    flex: 0 0 auto;
    max-width: min(25ch, 90%);
    height: var(--h-panel-hdr);
    border-inline-end: 1px solid var(--divider);
    padding-inline-end: var(--sp-2);
  }

  .tab.active {
    background: var(--state-selected);
    box-shadow: inset 0 -2px var(--selected-bar);
    --mark-halo: var(--state-selected);
  }

  .tab.over {
    outline: 1px solid var(--state-focus-ring);
    outline-offset: -1px;
  }

  button {
    color: var(--text-primary);
    border: 0;
    background: transparent;
    border-radius: var(--r-sm);
    font: inherit;
  }

  .tab:not(.active):hover {
    background: var(--state-hover);
    --mark-halo: var(--state-hover);
  }

  .select {
    min-width: 0;
    height: 100%;
    gap: var(--sp-3);
    padding-inline: var(--sp-4);
    font-size: var(--fs-dense);
  }

  .closed .select,
  .close {
    color: var(--text-secondary);
  }

  .close {
    justify-content: center;
    width: var(--h-button-sm);
    height: var(--h-button-sm);
    flex: none;
    padding: 0;
  }

  .close:hover {
    color: var(--text-primary);
  }

  .close :global(svg) {
    width: var(--panel-icon);
    height: var(--panel-icon);
  }
</style>
