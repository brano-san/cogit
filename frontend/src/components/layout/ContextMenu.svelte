<script lang="ts">
  import { tick } from "svelte";
  import MenuSurface from "$components/common/MenuSurface.svelte";
  import type { MenuRow } from "$lib/menu-nav";
  import { webMenus } from "$stores/web-menus.svelte";

  /** The page's context menu: what `popupContextMenu` opens when the page draws menus. It
      closes before the choice is sent, so the command meets the window with no modal layer
      of its own, as it does after a native menu. */
  const open = $derived(webMenus.context);

  async function pick(row: MenuRow) {
    webMenus.closeContext();
    await tick();
    void webMenus.run(row.id);
  }
</script>

{#if open}
  {#key open}
    <MenuSurface rows={open.rows} at={{ x: open.x, y: open.y }} onpick={pick} onclose={() => webMenus.closeContext()} />
  {/key}
{/if}
