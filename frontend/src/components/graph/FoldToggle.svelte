<script lang="ts">
  import Disclosure from "$components/common/Disclosure.svelte";

  /** A merge whose branch is folded into its row, or one opened to show it (#26). */
  interface Props {
    open: boolean;
    /** Commits the fold holds; only a folded merge knows it. */
    hidden: number;
    ontoggle: () => void;
  }

  let { open, hidden, ontoggle }: Props = $props();

  const label = $derived(
    open ? "Fold the merged branch" : `Show the ${hidden} ${hidden === 1 ? "commit" : "commits"} this merge brought in`,
  );
</script>

<span class="fold" title={label}>
  <Disclosure
    {open}
    {label}
    onclick={(event) => {
      event.stopPropagation();
      ontoggle();
    }}
  />
  {#if !open}<span class="count tabular">{hidden}</span>{/if}
</span>

<style>
  .fold {
    display: inline-flex;
    flex: 0 0 auto;
    align-items: center;
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }

  .count {
    padding-right: var(--sp-2);
  }
</style>
