<script lang="ts">
  import { edgeCursor, edgesFor } from "$lib/titlebar";
  import { windowControl } from "$lib/window-control";
  import { webMenus } from "$stores/web-menus.svelte";

  /** Thin invisible grips on every edge and corner of a window with no decorations: the
      window manager draws none, so the page asks it to start a resize. */
  const edges = $derived(edgesFor(webMenus.chrome, webMenus.frame));
</script>

{#each edges as edge (edge)}
  <div
    class="grip {edge}"
    style:cursor={edgeCursor(edge)}
    role="presentation"
    onpointerdown={(event) => {
      if (event.button !== 0) return;
      event.preventDefault();
      void windowControl.startResize(edge).catch(() => {});
    }}
  ></div>
{/each}

<style>
  .grip {
    position: fixed;
    z-index: 1003;
  }

  .North,
  .South {
    left: 8px;
    right: 8px;
    height: 4px;
  }

  .East,
  .West {
    top: 8px;
    bottom: 8px;
    width: 4px;
  }

  .North {
    top: 0;
  }

  .South {
    bottom: 0;
  }

  .East {
    right: 0;
  }

  .West {
    left: 0;
  }

  .NorthEast,
  .NorthWest,
  .SouthEast,
  .SouthWest {
    width: 8px;
    height: 8px;
  }

  .NorthEast {
    top: 0;
    right: 0;
  }

  .NorthWest {
    top: 0;
    left: 0;
  }

  .SouthEast {
    right: 0;
    bottom: 0;
  }

  .SouthWest {
    bottom: 0;
    left: 0;
  }
</style>
