<script lang="ts">
  import Icon from "./Icon.svelte";

  interface Props {
    kind?: "check" | "radio";
    checked: boolean;
  }

  let { kind = "check", checked }: Props = $props();
</script>

<span class="mark {kind}" class:checked aria-hidden="true"
  >{#if checked && kind === "check"}<Icon name="check" />{/if}</span
>

<style>
  .mark {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    width: 12px;
    height: 12px;
    --icon-size: 12px;
  }

  .mark.check :global(svg) {
    stroke-width: 1.8;
  }

  .mark.radio.checked {
    background: radial-gradient(circle, currentColor 0 3px, transparent 3.5px);
  }
</style>
