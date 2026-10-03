<script lang="ts">
  import { onMount, type Component } from "svelte";
  import { topRow } from "$lib/titlebar";
  import { webMenus } from "$stores/web-menus.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import Titlebar from "./Titlebar.svelte";

  /** What every window mounts: the page the window is for, under the row that is the
      titlebar or the menu bar when the page draws them (`window_chrome`), with the context
      menu above both. With neither, the page fills the window as it always did. */
  interface Props {
    page: Component;
    main?: boolean;
  }

  let { page: Page, main = false }: Props = $props();

  const row = $derived(topRow(webMenus.chrome, webMenus.frame, webMenus.model.length > 0));

  onMount(() => {
    void webMenus.boot();
  });
</script>

<div class="frame">
  {#if row !== "none"}<Titlebar showIcon={main} />{/if}
  <div class="body"><Page /></div>
</div>
<ContextMenu />

<style>
  .frame {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  /** A grid, so the page stretches to the row whatever the engine thinks of a percentage
      height inside a flex item. */
  .body {
    position: relative;
    z-index: 0;
    display: grid;
    grid-template-rows: minmax(0, 1fr);
    grid-template-columns: minmax(0, 1fr);
    flex: 1;
    min-height: 0;
  }
</style>
