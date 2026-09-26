<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import Checkbox from "$components/common/Checkbox.svelte";
  import Select from "$components/common/Select.svelte";
  import { followUp, problemOf, type DialogSpec, type Values } from "$lib/remote-dialogs";

  /** Draws any of the Remote ▸ Submodule, Subtree and LFS dialogs from its spec. */
  interface Props {
    spec: DialogSpec;
    link?: string;
    onsubmit: (values: Values) => void;
    onclose: () => void;
  }

  let { spec, link, onsubmit, onclose }: Props = $props();

  // svelte-ignore state_referenced_locally
  let values = $state<Values>({ ...spec.values });
  let touched = $state(new Set<string>());
  let tried = $state(false);
  let form: HTMLDivElement | undefined = $state();

  const problem = $derived(problemOf(spec, values));

  function edit(name: string, value: string | boolean) {
    values = followUp({ ...values, [name]: value }, name, touched);
    touched = new Set([...touched, name]);
  }

  function submit() {
    if (problem) {
      tried = true;
      form?.querySelector<HTMLElement>(`[name="${problem.field}"]`)?.focus();
      return;
    }
    onsubmit(values);
  }

  async function openLink() {
    if (!link) return;
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(link).catch(() => {});
  }
</script>

<Dialog title={spec.title} {onclose} onconfirm={submit} width="min(520px, 92vw)">
  <div class="form" bind:this={form}>
    <p class="intro">{spec.intro}</p>
    {#if link}
      <button type="button" class="link" onclick={() => void openLink()}>{link}</button>
    {/if}

    {#each spec.fields as field, index (field.name)}
      {#if field.kind === "text"}
        <label class="field">
          <span>{field.label}{field.required ? "" : " (optional)"}</span>
          <input
            type="text"
            name={field.name}
            list={field.suggestions?.length ? `remote-op-${field.name}` : undefined}
            placeholder={field.placeholder}
            value={values[field.name] as string}
            data-autofocus={index === 0 ? "" : undefined}
            oninput={(event) => edit(field.name, event.currentTarget.value)}
          />
          {#if field.suggestions?.length}
            <datalist id={`remote-op-${field.name}`}>
              {#each field.suggestions as suggestion (suggestion)}<option value={suggestion}></option>{/each}
            </datalist>
          {/if}
          {#if tried && problem?.field === field.name}
            <span class="problem" role="alert">{problem.message}</span>
          {:else if field.hint}
            <span class="hint">{field.hint}</span>
          {/if}
        </label>
      {:else if field.kind === "check"}
        <Checkbox
          checked={values[field.name] === true}
          label={field.label}
          onchange={(checked) => edit(field.name, checked)}
        />
      {:else}
        <div class="field">
          <span>{field.label}</span>
          <Select
            value={values[field.name] as string}
            label={field.label}
            options={field.options.map((option) => [option, option] as const)}
            onchange={(next) => edit(field.name, next)}
          />
        </div>
      {/if}
    {/each}
  </div>

  {#snippet footer()}
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="button" class="btn" class:primary={!spec.destructive} class:warning={spec.destructive} onclick={submit}>
      {spec.confirm}
    </button>
  {/snippet}
</Dialog>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-5);
    font-size: var(--fs-dense);
  }

  .intro {
    margin: 0;
    color: var(--text-secondary);
    line-height: 1.45;
  }

  .link {
    align-self: flex-start;
    padding: 0;
    background: none;
    border: 0;
    color: var(--link);
    font: inherit;
    text-decoration: underline;
    cursor: pointer;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .hint {
    color: var(--text-secondary);
  }

  .problem {
    color: var(--status-modify);
  }
</style>
