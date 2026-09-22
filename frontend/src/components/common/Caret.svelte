<script lang="ts">
  /** The caret of every dropdown and split button: the same size wherever a menu opens. */
  interface Props {
    open?: boolean;
  }

  let { open = false }: Props = $props();
</script>

<svg class="caret" class:open viewBox="0 0 10 10" aria-hidden="true"
  ><path d="M2 3.5 5 6.5 8 3.5" /></svg
>

<style>
  .caret {
    flex: none;
    width: var(--menu-caret);
    height: var(--menu-caret);
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
    transition: transform 0.12s ease-out;
  }

  .caret.open {
    transform: rotate(180deg);
  }
</style>
