<script lang="ts">
  interface Props {
    before: string | null;
    after: string | null;
    oldSize: number;
    newSize: number;
    mime: string;
  }

  let { before, after, oldSize, newSize, mime }: Props = $props();

  type Mode = "side" | "swipe" | "onion";

  let mode = $state<Mode>("side");
  let swipe = $state(50);
  let opacity = $state(50);

  function kb(bytes: number): string {
    return bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} KB`;
  }
</script>

<div class="image-diff">
  <div class="bar">
    <span class="mime">{mime}</span>
    <span class="sizes tabular">{kb(oldSize)} → {kb(newSize)}</span>
    <span class="grow"></span>
    {#each [["side", "Side by side"], ["swipe", "Swipe"], ["onion", "Onion skin"]] as [id, label] (id)}
      <button type="button" class:active={mode === id} onclick={() => (mode = id as Mode)}>
        {label}
      </button>
    {/each}
  </div>

  {#if !before && !after}
    <p class="message">Neither side has an image to show.</p>
  {:else if mode === "side"}
    <div class="side">
      <figure>
        <figcaption>Before</figcaption>
        {#if before}<img src={before} alt="Before" />{:else}<p class="message">Added</p>{/if}
      </figure>
      <figure>
        <figcaption>After</figcaption>
        {#if after}<img src={after} alt="After" />{:else}<p class="message">Deleted</p>{/if}
      </figure>
    </div>
  {:else if mode === "swipe"}
    <div class="stage">
      <div class="stack">
        {#if before}<img src={before} alt="Before" />{/if}
        {#if after}
          <img class="over" src={after} alt="After" style:clip-path="inset(0 0 0 {swipe}%)" />
        {/if}
      </div>
      <input type="range" min="0" max="100" bind:value={swipe} aria-label="Swipe position" />
    </div>
  {:else}
    <div class="stage">
      <div class="stack">
        {#if before}<img src={before} alt="Before" />{/if}
        {#if after}<img class="over" src={after} alt="After" style:opacity={opacity / 100} />{/if}
      </div>
      <input type="range" min="0" max="100" bind:value={opacity} aria-label="Overlay opacity" />
    </div>
  {/if}
</div>

<style>
  .image-diff {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex: 0 0 auto;
    padding: var(--sp-3) var(--sp-4);
    border-bottom: 1px solid var(--divider);
    font-size: var(--fs-dense);
  }

  .mime,
  .sizes {
    color: var(--text-secondary);
    font-size: 11px;
  }

  .grow {
    flex: 1 1 auto;
  }

  .bar button {
    height: var(--h-button-sm);
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }

  .bar button.active {
    border-color: var(--status-ref);
    color: var(--status-ref);
  }

  .side {
    display: flex;
    gap: var(--sp-5);
    flex: 1 1 auto;
    min-height: 0;
    padding: var(--sp-5);
    overflow: auto;
  }

  figure {
    flex: 1 1 50%;
    min-width: 0;
    margin: 0;
  }

  figcaption {
    margin-bottom: var(--sp-3);
    color: var(--text-secondary);
    font-size: var(--fs-header);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .stage {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    flex: 1 1 auto;
    min-height: 0;
    padding: var(--sp-5);
    overflow: auto;
  }

  .stack {
    position: relative;
    display: inline-block;
  }

  .over {
    position: absolute;
    inset: 0;
  }

  img {
    display: block;
    max-width: 100%;
    /* A checkerboard makes transparency visible instead of blending into the panel. */
    background-image:
      linear-gradient(45deg, var(--surface-raised) 25%, transparent 25%),
      linear-gradient(-45deg, var(--surface-raised) 25%, transparent 25%),
      linear-gradient(45deg, transparent 75%, var(--surface-raised) 75%),
      linear-gradient(-45deg, transparent 75%, var(--surface-raised) 75%);
    background-size: 16px 16px;
    background-position:
      0 0,
      0 8px,
      8px -8px,
      -8px 0;
  }

  .message {
    margin: 0;
    padding: var(--sp-5);
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }
</style>
