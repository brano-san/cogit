<script lang="ts">
  import { diffEdit } from "$stores/diff-edit.svelte";

  /** What the editor of the working file says above it, in the diff and in the Edit window:
      the file moved on under it (Reload or Keep Mine), or the save failed. */
  interface Props {
    /** Keep Mine: the editor's text over whatever is on disk now. */
    onkeep: () => void;
  }

  let { onkeep }: Props = $props();
</script>

{#if diffEdit.changedOnDisk}
  <p class="banner" role="alert">
    The file changed on disk since you opened it.
    <button type="button" class="btn sm" onclick={() => void diffEdit.reload()} title="Take the file on disk; your edits go"
      >Reload</button
    >
    <button type="button" class="btn sm" onclick={onkeep} title="Write your text over the file on disk">Keep Mine</button>
  </p>
{/if}
{#if diffEdit.error}
  <p class="banner error" role="alert">{diffEdit.error}</p>
{/if}

<style>
  .banner {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    flex: 0 0 auto;
    margin: 0;
    padding: var(--sp-2) var(--sp-4);
    background: var(--badge-warning-bg);
    color: var(--badge-warning-fg);
    font-size: var(--fs-dense);
  }

  .banner.error {
    background: var(--badge-error-bg);
    color: var(--badge-error-fg);
    user-select: text;
  }
</style>
