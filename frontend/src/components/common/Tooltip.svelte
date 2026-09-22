<script lang="ts">
  import type { Snippet } from "svelte";

  /** A tooltip with a shortcut beside it. `TooltipLayer` draws it, the same bubble and the
      same delay as every `title` in the window (doc/12-risks.md, R-181). */
  interface Props {
    label: string;
    hint?: string;
    below?: boolean;
    children: Snippet;
  }

  let { label, hint, below = false, children }: Props = $props();
</script>

<span
  class="wrap"
  data-tip={label}
  data-tip-hint={hint}
  data-tip-below={below ? "" : undefined}
>
  {@render children()}
</span>

<style>
  .wrap {
    display: inline-flex;
  }
</style>
