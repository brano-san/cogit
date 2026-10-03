<script lang="ts">
  import { commands } from "$lib/ipc/bindings";
  import { maximizeButton, pastSlop, startsDrag } from "$lib/titlebar";
  import { windowControl } from "$lib/window-control";
  import { webMenus } from "$stores/web-menus.svelte";
  import MenuBar from "./MenuBar.svelte";
  import icon from "../../../../src-tauri/icons/32x32.png";

  /** The row on top of a window with no decorations of its own, or the bare menu bar when
      only the menus are the page's. Drag moves the window, a double click maximizes it,
      the buttons are Windows': minimize, maximize or restore, close. Resizing needs nothing
      here: Tauri grips the edges of an undecorated window itself (R-728). */
  interface Props {
    /** The application icon at the left: the main window only. */
    showIcon?: boolean;
  }

  let { showIcon = false }: Props = $props();

  const own = $derived(webMenus.chrome.customTitlebar);
  const button = $derived(maximizeButton(webMenus.frame));
  let bar: HTMLElement | undefined = $state();
  /** Where the primary button went down on the empty bar; the drag starts once it moves. */
  let pressed: { x: number; y: number } | null = null;

  function press(event: PointerEvent) {
    pressed = own && bar && event.detail <= 1 && startsDrag(event, bar) ? { x: event.clientX, y: event.clientY } : null;
    // A quick pull off the bar still counts; released below before the manager takes over,
    // or the page under the bar would keep losing its pointer to it.
    if (pressed) bar?.setPointerCapture(event.pointerId);
  }

  function move(event: PointerEvent) {
    if (!pressed) return;
    if ((event.buttons & 1) === 0) return void (pressed = null);
    if (!pastSlop(pressed, { x: event.clientX, y: event.clientY })) return;
    pressed = null;
    bar?.releasePointerCapture(event.pointerId);
    void windowControl.startDragging().catch(() => {});
  }

  function toggle(event: MouseEvent) {
    const target = event.target as Element | null;
    if (!own || target?.closest("button, [data-no-drag]")) return;
    void windowControl.toggleMaximize().catch(() => {});
  }
</script>

<header
  class="titlebar"
  class:own
  bind:this={bar}
  role="presentation"
  onpointerdown={press}
  onpointermove={move}
  onpointerup={() => (pressed = null)}
  ondblclick={toggle}
>
  {#if own && showIcon}<img class="icon" src={icon} alt="" width="16" height="16" />{/if}
  <MenuBar />
  {#if own}
    <span class="title">{webMenus.title}</span>
    <div class="controls" data-no-drag>
      <button type="button" class="control" aria-label="Minimize" title="Minimize" onclick={() => void windowControl.minimize().catch(() => {})}>
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 5.5h8" /></svg>
      </button>
      <button type="button" class="control" aria-label={button.title} title={button.title} onclick={() => void windowControl.toggleMaximize().catch(() => {})}>
        {#if button.glyph === "restore"}
          <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M2.5 3.5h5v5h-5zM3.5 3.5v-2h5v5h-1" /></svg>
        {:else}
          <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1.5 1.5h7v7h-7z" /></svg>
        {/if}
      </button>
      <button type="button" class="control close" aria-label="Close" title="Close" onclick={() => void commands.closeThisWindow().catch(() => {})}>
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1.5 1.5l7 7M8.5 1.5l-7 7" /></svg>
      </button>
    </div>
  {/if}
</header>

<style>
  .titlebar {
    position: relative;
    z-index: 1001;
    display: flex;
    align-items: center;
    flex: none;
    height: var(--h-menubar);
    background: var(--titlebar-bg);
    color: var(--fg-primary);
    border-bottom: 1px solid var(--border);
    user-select: none;
  }

  .icon {
    flex: none;
    margin: 0 var(--sp-3) 0 var(--sp-4);
    pointer-events: none;
  }

  .title {
    flex: 1;
    min-width: 0;
    padding: 0 var(--sp-5);
    overflow: hidden;
    color: var(--fg-secondary);
    font-size: var(--fs-dense);
    text-align: center;
    text-overflow: ellipsis;
    white-space: nowrap;
    pointer-events: none;
  }

  .controls {
    display: flex;
    align-self: stretch;
    flex: none;
  }

  .control {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 46px;
    padding: 0;
    background: none;
    color: var(--fg-primary);
    border: 0;
    border-radius: 0;
  }

  .control svg {
    width: 10px;
    height: 10px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1;
  }

  .control:hover {
    background: var(--bg-hover);
  }

  .control.close:hover {
    background: var(--status-danger);
    color: var(--fg-on-accent);
  }
</style>
