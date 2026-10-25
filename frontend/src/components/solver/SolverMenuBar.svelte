<script lang="ts">
  import { popupContextMenu } from "$lib/ipc";
  import { BAR_MENUS, solverBarMenu, type BarMenu, type SolverBarState } from "$lib/solver-menu";

  /** File, Edit, View, Go To, Window: each opens as a native popup under its title, and a
      row comes back as a `cogit-menu` action like the context menu's. A child window has no
      native menu bar of its own (CLAUDE.md, Windows), so this one is drawn by the page. */
  interface Props {
    label: string;
    state: SolverBarState;
  }

  let { label, state }: Props = $props();

  async function open(menu: BarMenu, event: MouseEvent) {
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    await popupContextMenu(solverBarMenu(label, menu, state), box.left, box.bottom).catch(() => {});
  }
</script>

<nav class="menubar" aria-label="Conflict Solver menu">
  {#each BAR_MENUS as menu (menu)}
    <button type="button" class="title" onclick={(event) => void open(menu, event)}>{menu}</button>
  {/each}
</nav>

<style>
  .menubar {
    display: flex;
    flex: 0 0 auto;
    gap: 1px;
    padding: 1px var(--sp-2);
    background: var(--chrome-bg);
    border-bottom: 1px solid var(--chrome-border);
    user-select: none;
  }

  .title {
    padding: 2px var(--sp-4);
    background: none;
    border: 0;
    border-radius: var(--r-sm);
    color: var(--text-primary);
    font: inherit;
    font-size: var(--fs-dense);
    cursor: default;
  }

  .title:hover {
    background: var(--state-hover);
  }
</style>
