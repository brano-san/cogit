<script lang="ts">
  import Checkbox from "$components/common/Checkbox.svelte";
  import Radio from "$components/common/Radio.svelte";

  /** One option: a short label with a `fg.muted` hint under it. Checkbox (default) binds
      `checked`; radios of one choice share `name` and answer through `onchange`. */
  interface Props {
    label: string;
    hint?: string;
    kind?: "checkbox" | "radio";
    checked?: boolean;
    name?: string;
    disabled?: boolean;
    onchange?: (checked: boolean) => void;
  }

  let {
    label,
    hint,
    kind = "checkbox",
    checked = $bindable(false),
    name,
    disabled = false,
    onchange,
  }: Props = $props();
</script>

{#snippet text()}
  <span class="text">
    <span class="label">{label}</span>
    {#if hint}<span class="hint">{hint}</span>{/if}
  </span>
{/snippet}

<div class="option">
  {#if kind === "radio"}
    <Radio {checked} {name} {disabled} onchange={() => onchange?.(true)}>{@render text()}</Radio>
  {:else}
    <Checkbox bind:checked {disabled} {onchange} wide>{@render text()}</Checkbox>
  {/if}
</div>

<style>
  .option {
    display: flex;
    font-size: var(--fs-dense);
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    min-width: 0;
  }

  .label {
    overflow-wrap: anywhere;
  }

  .hint {
    color: var(--text-muted);
    line-height: 1.4;
    overflow-wrap: anywhere;
  }
</style>
