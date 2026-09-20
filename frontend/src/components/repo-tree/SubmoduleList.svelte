<script lang="ts">
  import { shortOid } from "$lib/format";
  import type { Submodule } from "$lib/ipc";

  interface Props {
    modules: readonly Submodule[];
    onopen: (module: Submodule) => void;
    onupdate: (module: Submodule) => void;
  }

  let { modules, onopen, onupdate }: Props = $props();

  const LABEL: Record<Submodule["state"], string> = {
    inSync: "",
    diverged: "diverged",
    notInitialised: "not initialised",
  };
</script>

{#if modules.length > 0}
  <div class="section">
    <div class="section-header">Submodules ({modules.length})</div>
    {#each modules as module (module.path)}
      <div class="row {module.state}" title="{module.url} — recorded {module.recorded}">
        <span class="marker" aria-hidden="true">⊞</span>
        <span class="name truncate">{module.path}</span>
        {#if LABEL[module.state]}<span class="state">{LABEL[module.state]}</span>{/if}
        <span class="oid mono tabular">{shortOid(module.recorded)}</span>
        {#if module.state !== "inSync"}
          <span
            class="act"
            role="button"
            tabindex="-1"
            title="Check out the recorded commit"
            onclick={() => onupdate(module)}
            onkeydown={(e) => e.key === "Enter" && onupdate(module)}>Update</span
          >
        {:else}
          <span
            class="act"
            role="button"
            tabindex="-1"
            title="Open as its own repository"
            onclick={() => onopen(module)}
            onkeydown={(e) => e.key === "Enter" && onopen(module)}>Open</span
          >
        {/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  .section {
    padding-bottom: var(--sp-4);
  }

  .section-header {
    padding: var(--sp-3) var(--sp-5) var(--sp-2, 3px);
    color: var(--text-secondary);
    font-size: var(--fs-header);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    height: 22px;
    padding: 0 var(--sp-5);
    font-size: var(--fs-dense);
    white-space: nowrap;
  }

  .row:hover {
    background: var(--state-hover);
  }

  .marker {
    flex: 0 0 auto;
    width: 10px;
    color: var(--text-secondary);
  }

  .name {
    flex: 1 1 auto;
    min-width: 0;
  }

  .state {
    flex: 0 0 auto;
    color: var(--status-modify);
    font-size: 10px;
  }

  .row.notInitialised .state {
    color: var(--text-secondary);
  }

  .oid {
    flex: 0 0 auto;
    color: var(--text-secondary);
    font-size: 10px;
  }

  .act {
    flex: 0 0 auto;
    padding: 0 var(--sp-3);
    color: var(--text-secondary);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    opacity: 0;
    cursor: default;
  }

  .row:hover .act {
    opacity: 1;
  }

  .act:hover {
    color: var(--status-ref);
  }
</style>
