<script lang="ts">
  import Checkbox from "$components/common/Checkbox.svelte";
  import Radio from "$components/common/Radio.svelte";

  /** One option: a short label with a `fg.muted` hint under it. Checkbox (default) binds
      `checked`; radios of one choice share `name` and answer through `onchange`. A dense list
      puts the explanation in `tip` (the label's tooltip), a count in `note` beside it and the
      git flag the option adds in `flag`, on the right. */
  interface Props {
    label: string;
    hint?: string;
    tip?: string;
    note?: string;
    flag?: string;
    kind?: "checkbox" | "radio";
    checked?: boolean;
    name?: string;
    disabled?: boolean;
    onchange?: (checked: boolean) => void;
  }

  let {
    label,
    hint,
    tip,
    note,
    flag,
    kind = "checkbox",
    checked = $bindable(false),
    name,
    disabled = false,
    onchange,
  }: Props = $props();
</script>

{#snippet text()}
  <span class="text">
    <span class="line">
      <span class="label" title={tip}>{label}</span>
      {#if note}<span class="note">{note}</span>{/if}
      {#if flag}<span class="flag">{flag}</span>{/if}
    </span>
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
    flex: 1 1 auto;
    flex-direction: column;
    gap: var(--sp-1);
    min-width: 0;
  }

  .line {
    display: flex;
    flex-wrap: wrap;
    column-gap: var(--sp-2);
    min-width: 0;
  }

  .label {
    overflow-wrap: anywhere;
  }

  .note {
    color: var(--text-muted);
  }

  .flag {
    margin-left: auto;
    color: var(--text-muted);
    font-family: var(--font-mono);
  }

  .hint {
    color: var(--text-muted);
    line-height: 1.4;
    overflow-wrap: anywhere;
  }
</style>
