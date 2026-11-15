<script lang="ts">
  import Checkbox from "$components/common/Checkbox.svelte";
  import { globalNetworkSettings, setGlobalNetworkSetting } from "$lib/ipc";
  import { NETWORK_FIELDS, changes, ticked, toWrite, type GlobalValues } from "$lib/network-settings";
  import type { SettingField } from "$lib/repo-settings";
  import { errors } from "$stores/errors.svelte";

  /** Preferences ▸ Network (F-942): written to the user's global Git config the moment a
      field is left or ticked, so Cancel does not take it back. */
  let values = $state.raw<GlobalValues | null>(null);

  $effect(() => {
    void globalNetworkSettings()
      .then((read) => (values = new Map(read.map((entry) => [entry.key, entry.value]))))
      .catch((err: unknown) => errors.report(err, "Could not read the global Git config"));
  });

  function valueOf(field: SettingField): string | null {
    return values?.get(field.key) ?? null;
  }

  async function write(field: SettingField, input: string | boolean) {
    if (!values) return;
    const next = toWrite(field, input);
    if (!changes(values, field.key, next)) return;
    try {
      await setGlobalNetworkSetting(field.key, next);
      values = new Map(values).set(field.key, next);
    } catch (err) {
      errors.report(err, `Could not save ${field.key}`);
    }
  }

  async function browse(field: SettingField, title: string) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({ title, multiple: false, directory: false });
    if (typeof picked === "string") await write(field, picked);
  }
</script>

<p class="lead">
  Written to your global Git config (git config --global) as soon as a field is left; Cancel does not undo it. A
  repository can override each one in Repository ▸ Settings ▸ Network.
</p>

{#if values === null}
  <p class="hint">Reading the global Git config…</p>
{:else}
  {#each NETWORK_FIELDS as field (field.key)}
    {@const control = field.control}
    {#if control.kind === "bool"}
      <div class="row check">
        <Checkbox checked={ticked(field, valueOf(field))} label={field.label} onchange={(checked) => void write(field, checked)} />
      </div>
    {:else if control.kind === "text" || control.kind === "path"}
      <div class="row">
        <span>{field.label}</span>
        <div class="path">
          {#key valueOf(field)}
            <input
              type="text"
              class="text mono"
              spellcheck="false"
              aria-label={field.label}
              placeholder={control.placeholder}
              value={valueOf(field) ?? ""}
              onchange={(event) => void write(field, event.currentTarget.value)}
            />
          {/key}
          {#if control.kind === "path"}
            <button type="button" class="btn browse" title="Browse…" aria-label={control.title} onclick={() => void browse(field, control.title)}
              >...</button
            >
          {/if}
        </div>
      </div>
    {/if}
    {#if field.hint}<p class="hint">{field.hint}</p>{/if}
  {/each}
{/if}

<style>
  .lead {
    margin: 0 0 var(--sp-5);
    color: var(--text-secondary);
    line-height: 1.45;
  }

  .row {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    margin-top: var(--sp-4);
  }

  .path {
    display: flex;
    gap: var(--sp-2);
  }

  .path input {
    flex: 1 1 auto;
    min-width: 0;
  }

  .hint {
    margin: var(--sp-2) 0 0;
    color: var(--text-secondary);
    line-height: 1.4;
    overflow-wrap: anywhere;
  }
</style>
