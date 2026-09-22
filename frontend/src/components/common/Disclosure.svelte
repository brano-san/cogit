<script lang="ts">
  /** The triangle every tree opens a node with: one size, one hit area, turned when open. */
  interface Props {
    open?: boolean;
    empty?: boolean;
    label?: string;
    /** Absent when the whole row toggles: the triangle is then a picture, not a button. */
    onclick?: (event: MouseEvent) => void;
  }

  let { open = false, empty = false, label, onclick }: Props = $props();
</script>

{#snippet glyph()}
  <svg class="glyph" class:open viewBox="0 0 10 10" aria-hidden="true"
    ><path d="M3 1.5 7.5 5 3 8.5Z" /></svg
  >
{/snippet}

{#if empty}
  <span class="disclosure" aria-hidden="true"></span>
{:else if onclick}
  <button type="button" class="disclosure" aria-label={label} aria-expanded={open} {onclick}
    >{@render glyph()}</button
  >
{:else}
  <span class="disclosure" aria-hidden="true">{@render glyph()}</span>
{/if}

<style>
  .disclosure {
    display: inline-flex;
    flex: 0 0 var(--disclosure-hit);
    align-items: center;
    justify-content: center;
    width: var(--disclosure-hit);
    height: var(--disclosure-hit);
    padding: 0;
    background: none;
    border: 0;
    color: var(--text-secondary);
    cursor: default;
  }

  button.disclosure:hover {
    color: var(--text-primary);
  }

  .glyph {
    width: var(--disclosure-glyph);
    height: var(--disclosure-glyph);
    fill: currentColor;
    transition: transform 0.12s ease-out;
  }

  .glyph.open {
    transform: rotate(90deg);
  }
</style>
