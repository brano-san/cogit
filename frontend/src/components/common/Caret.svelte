<script lang="ts">
  /** The disclosure triangle every tree in the app opens with. One component, because a
      12px caret in one panel and a 20px one in the next is the same control twice. */
  interface Props {
    open?: boolean;
    /** Nothing to open: an empty box of the same size, so the rows still line up. */
    empty?: boolean;
    label?: string;
    /** Absent when the whole row toggles: the triangle is then a picture, not a button. */
    onclick?: (event: MouseEvent) => void;
  }

  let { open = false, empty = false, label, onclick }: Props = $props();
</script>

{#if empty}
  <span class="caret" aria-hidden="true"></span>
{:else if onclick}
  <button type="button" class="caret" aria-label={label} {onclick}>{open ? "▾" : "▸"}</button>
{:else}
  <span class="caret" aria-hidden="true">{open ? "▾" : "▸"}</span>
{/if}

<style>
  /* 20×20 is the click target; the glyph inside it is 11px, which is the smallest a
     triangle still reads as a triangle at 100% scale. */
  .caret {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 20px;
    width: 20px;
    height: 20px;
    padding: 0;
    background: none;
    border: 0;
    color: var(--text-secondary);
    font: inherit;
    font-size: 11px;
    line-height: 1;
    cursor: default;
  }

  button.caret:hover {
    color: var(--text-primary);
  }
</style>
