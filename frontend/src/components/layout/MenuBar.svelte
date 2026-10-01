<script lang="ts">
  import { tick } from "svelte";
  import MenuSurface from "$components/common/MenuSurface.svelte";
  import { modals } from "$lib/modal-stack";
  import { mnemonics, type MenuRow } from "$lib/menu-nav";
  import { opensBar } from "$lib/web-menu";
  import { webMenus } from "$stores/web-menus.svelte";

  /** The application's menus, drawn by the page: the titles in a row, a drop-down under the
      open one, Alt+letter and F10 to reach them from the keyboard. */
  const menus = $derived(webMenus.bar);
  const marks = $derived(mnemonics(menus.map((menu) => menu.label)));

  let open = $state<{ index: number; keyboard: boolean } | null>(null);
  let alt = $state(false);
  let titles: HTMLElement[] = $state.raw([]);
  /** The field that had the focus: Cut, Copy and Paste act on it, not on the bar. */
  let restore: HTMLElement | null = null;

  function openMenu(index: number, keyboard: boolean) {
    if (open === null) {
      const active = document.activeElement;
      restore = active instanceof HTMLElement && active !== document.body ? active : null;
    }
    open = { index, keyboard };
  }

  function close() {
    open = null;
  }

  async function pick(row: MenuRow) {
    const back = restore;
    open = null;
    await tick();
    void webMenus.run(row.id, back);
  }

  function boxOf(index: number) {
    const box = titles[index]?.getBoundingClientRect();
    return box ? { left: box.left, top: box.top, right: box.right, bottom: box.bottom } : { left: 0, top: 0, right: 0, bottom: 0 };
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Alt") alt = true;
    if (modals.any || event.defaultPrevented || menus.length === 0 || !opensBar(event)) return;
    if (event.key === "F10") {
      event.preventDefault();
      openMenu(0, true);
      return;
    }
    const index = marks.findIndex((at, i) => at !== null && menus[i]!.label[at]!.toLowerCase() === event.key.toLowerCase());
    if (index < 0) return;
    event.preventDefault();
    openMenu(index, true);
  }

  function onkeyup(event: KeyboardEvent) {
    if (event.key === "Alt") alt = false;
  }
</script>

<svelte:window {onkeydown} {onkeyup} onblur={() => (alt = false)} />

{#if menus.length > 0}
  <div class="bar" role="menubar" aria-label="Application menu">
    {#each menus as menu, index (menu.id)}
      <button
        type="button"
        role="menuitem"
        class="title"
        class:open={open?.index === index}
        aria-haspopup="menu"
        aria-expanded={open?.index === index}
        data-no-drag
        bind:this={titles[index]}
        onpointerdown={(event) => event.preventDefault()}
        onclick={() => (open?.index === index ? close() : openMenu(index, false))}
        onpointerenter={() => open !== null && open.index !== index && openMenu(index, false)}
      >
        {#if marks[index] !== null}
          {menu.label.slice(0, marks[index]!)}<span class:mark={alt || open !== null}>{menu.label[marks[index]!]}</span>{menu.label.slice(marks[index]! + 1)}
        {:else}
          {menu.label}
        {/if}
      </button>
    {/each}
  </div>

  {#if open !== null}
    {#key open.index}
      <MenuSurface
        rows={menus[open.index]!.children}
        at={{ box: boxOf(open.index) }}
        bar
        focusFirst={open.keyboard}
        label={menus[open.index]!.label}
        onpick={pick}
        onclose={close}
        onbar={(step) => openMenu((open!.index + step + menus.length) % menus.length, true)}
      />
    {/key}
  {/if}
{/if}

<style>
  .bar {
    display: flex;
    align-items: stretch;
    height: 100%;
    flex: none;
  }

  .title {
    display: flex;
    align-items: center;
    padding: 0 var(--sp-4);
    background: none;
    color: var(--fg-primary);
    border: 0;
    border-radius: 0;
    font: inherit;
    font-size: var(--fs-ui);
    cursor: default;
  }

  .title:hover {
    background: var(--bg-hover);
  }

  /* The open title and its drop-down are one surface. */
  .title.open {
    background: var(--bg-elevated);
  }

  .mark {
    text-decoration: underline;
  }
</style>
