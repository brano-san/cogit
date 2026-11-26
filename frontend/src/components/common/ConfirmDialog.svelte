<script lang="ts">
  import Icon from "$components/common/Icon.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import Checkbox from "$components/common/Checkbox.svelte";
  import Select from "$components/common/Select.svelte";
  import type { ConfirmChoice } from "$stores/confirm.svelte";

  interface Props {
    title: string;
    message: string;
    confirm: string;
    cancel?: string;
    warning?: boolean;
    items?: readonly string[];
    fields?: readonly (readonly [string, string])[];
    note?: string;
    option?: string;
    choice?: ConfirmChoice;
    checks?: readonly (readonly [string, string])[];
    alternative?: string;
    onanswer: (yes: boolean, checked: boolean, choice?: string, alternative?: boolean, checks?: string[]) => void;
  }

  let { title, message, confirm, cancel = "Cancel", warning = false, items, fields, note, option, choice, checks, alternative, onanswer }: Props = $props();
  let checked = $state(false);
  // svelte-ignore state_referenced_locally
  let picked = $state(choice?.value);
  let selected = $state.raw<string[]>([]);

  function accept(alternate = false) {
    onanswer(true, checked, picked, alternate, selected);
  }
</script>

<Dialog
  {title}
  onclose={() => onanswer(false, false)}
  onconfirm={() => accept()}
  width={fields ? "min(500px, 90vw)" : "min(460px, 90vw)"}
>
  <p class="message">{message}</p>
  {#if fields && fields.length > 0}
    <dl class="fields">
      {#each fields as [label, value] (label)}
        <dt>{label}:</dt>
        <dd class="mono">{value}</dd>
      {/each}
    </dl>
  {/if}
  {#if items && items.length > 0}
    <ul class="items mono">
      {#each items as item (item)}<li>{item}</li>{/each}
    </ul>
  {/if}
  {#if choice && picked !== undefined}
    <label class="option">
      {choice.label}
      <Select
        value={picked}
        options={choice.options}
        label={choice.label}
        disabled={choice.disabledBy !== undefined && selected.includes(choice.disabledBy)}
        onchange={(value) => (picked = value)}
      />
    </label>
  {/if}
  {#if option}
    <div class="option"><Checkbox bind:checked label={option} /></div>
  {/if}
  {#each checks ?? [] as [id, label] (id)}
    <div class="option">
      <Checkbox
        {label}
        checked={selected.includes(id)}
        onchange={(enabled) => (selected = enabled ? [...selected, id] : selected.filter((key) => key !== id))}
      />
    </div>
  {/each}
  {#if note}
    <p class="note">
      <Icon name="warning" />
      <span>{note}</span>
    </p>
  {/if}

  {#snippet footer()}
    <!-- A destructive question starts on Cancel: Enter must not be the way work is lost. -->
    <button class="btn" type="button" data-autofocus={warning || undefined} onclick={() => onanswer(false, false)}>
      {cancel}
    </button>
    {#if alternative}
      <button type="button" class="btn" onclick={() => accept(true)}>{alternative}</button>
    {/if}
    <button
      type="button"
      class="btn"
      class:primary={!warning}
      class:warning
      data-autofocus={!warning || undefined}
      onclick={() => accept()}
    >
      {confirm}
    </button>
  {/snippet}
</Dialog>

<style>
  .message {
    margin: 0;
    font-size: var(--fs-dense);
    line-height: 1.5;
    white-space: pre-line;
    overflow-wrap: anywhere;
  }
  .items {
    max-height: 12em;
    margin: 10px 0 0;
    padding: var(--sp-2) var(--sp-3);
    overflow-y: auto;
    list-style: none;
    font-size: var(--fs-dense);
    background: var(--surface-input);
    border: 1px solid var(--divider);
    border-radius: var(--r-sm);
    overflow-wrap: anywhere;
    user-select: text;
  }
  .fields {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: var(--sp-2) var(--sp-4);
    margin: var(--sp-5) 0 0;
    font-size: var(--fs-dense);
  }
  .fields dt {
    color: var(--text-secondary);
  }
  .fields dd {
    margin: 0;
    color: var(--text-primary);
    overflow-wrap: anywhere;
    user-select: text;
  }
  .note {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
    margin: var(--sp-5) 0 0;
    color: var(--text-secondary);
    font-size: var(--fs-dense);
    line-height: 1.5;
  }
  .note :global(svg) {
    flex: none;
    width: 14px;
    height: 14px;
    margin-top: max(calc((1lh - 14px) / 2), 0px);
    fill: none;
    stroke: var(--status-modify);
    stroke-width: 1.3;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .option {
    display: flex;
    gap: 6px;
    align-items: center;
    margin-top: 10px;
    font-size: var(--fs-dense);
  }
</style>
