<script lang="ts">
  import { visibleLines, type InvestigateCommand, type Menu } from "$lib/investigate/menu";

  /** The Investigate window's own menu bar, drawn in the page (R-283). */
  interface Props {
    menus: readonly Menu[];
    /** The label of the open menu; bound so the window can close it on Esc. */
    open: string | null;
    stateOf: (id: InvestigateCommand) => { enabled: boolean; checked: boolean };
    oncommand: (id: InvestigateCommand) => void;
  }

  let { menus, open = $bindable(null), stateOf, oncommand }: Props = $props();

  function pick(id: InvestigateCommand) {
    open = null;
    oncommand(id);
  }
</script>

{#if open !== null}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="backdrop" onclick={() => (open = null)}></div>
{/if}

<div class="menubar" role="menubar">
  {#each menus as menu (menu.label)}
    <div class="top">
      <button
        type="button"
        role="menuitem"
        aria-haspopup="menu"
        aria-expanded={open === menu.label}
        class:open={open === menu.label}
        onclick={() => (open = open === menu.label ? null : menu.label)}
        onpointerenter={() => {
          if (open !== null) open = menu.label;
        }}>{menu.label}</button
      >
      {#if open === menu.label}
        <div class="drop" role="menu" aria-label={menu.label}>
          {#each visibleLines(menu.items) as line, index (index)}
            {#if line === "separator"}
              <div class="separator" role="separator"></div>
            {:else}
              {@const state = stateOf(line.id)}
              <button
                type="button"
                role={line.mark === "check"
                  ? "menuitemcheckbox"
                  : line.mark === "radio"
                    ? "menuitemradio"
                    : "menuitem"}
                aria-checked={line.mark ? state.checked : undefined}
                disabled={!state.enabled}
                onclick={() => pick(line.id)}
              >
                <span class="mark">{state.checked ? (line.mark === "radio" ? "●" : "✓") : ""}</span>
                <span class="label">{line.label}</span>
                <span class="key">{line.shortcut ?? ""}</span>
              </button>
            {/if}
          {/each}
        </div>
      {/if}
    </div>
  {/each}
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
  }

  .menubar {
    position: relative;
    z-index: 41;
    display: flex;
    flex: none;
    height: var(--h-menubar);
    padding: 0 var(--sp-3);
    background: var(--titlebar-bg);
    border-bottom: 1px solid var(--titlebar-border);
    font-size: var(--fs-dense);
  }

  .top {
    position: relative;
  }

  .top > button {
    height: 100%;
    padding: 0 var(--sp-5);
    background: none;
    color: var(--text-primary);
    border: 0;
    font: inherit;
    cursor: default;
  }

  .top > button:hover,
  .top > button.open {
    background: var(--state-hover);
  }

  .drop {
    position: absolute;
    top: 100%;
    left: 0;
    display: flex;
    flex-direction: column;
    min-width: 240px;
    padding: var(--sp-2) 0;
    background: var(--surface-raised);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    box-shadow: var(--shadow-popover);
  }

  .drop button {
    display: grid;
    grid-template-columns: 18px 1fr auto;
    align-items: center;
    gap: var(--sp-3);
    height: var(--h-row);
    padding: 0 var(--sp-5) 0 var(--sp-3);
    background: none;
    color: var(--text-primary);
    border: 0;
    font: inherit;
    text-align: left;
    cursor: default;
  }

  .drop button:hover:not(:disabled) {
    background: var(--state-hover);
  }

  .drop button:disabled {
    color: var(--text-secondary);
    opacity: 0.6;
  }

  .mark {
    text-align: center;
    font-size: 10px;
  }

  .key {
    padding-left: var(--sp-6);
    color: var(--text-secondary);
  }

  .separator {
    height: 1px;
    margin: var(--sp-2) var(--sp-3);
    background: var(--divider);
  }
</style>
