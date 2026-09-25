<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import Checkbox from "$components/common/Checkbox.svelte";
  import Radio from "$components/common/Radio.svelte";
  import type { RepoId } from "$lib/ipc";
  import { repoSettings, writeRepoSettings, type RepoSetting } from "$lib/ipc/remote-ops";
  import {
    FIELDS,
    TABS,
    changes,
    effective,
    initialDrafts,
    isOn,
    normalise,
    origin,
    separatorProblem,
    storage,
    type SettingField,
    type SettingTab,
  } from "$lib/repo-settings";
  import { errors } from "$stores/errors.svelte";

  /** Repository ▸ Settings… (#42). */
  interface Props {
    repo: RepoId;
    name: string;
    onsaved: () => Promise<void>;
    onclose: () => void;
  }

  let { repo, name, onsaved, onclose }: Props = $props();

  let read = $state.raw<RepoSetting[] | null>(null);
  let drafts = $state.raw<Map<string, string | null>>(new Map());
  let tab = $state<SettingTab>("user");
  let saving = $state(false);

  const inherited = $derived(new Map((read ?? []).map((entry) => [entry.key, entry.inherited])));
  const shown = $derived(FIELDS.filter((field) => field.tab === tab));
  const problem = $derived(separatorProblem(drafts.get("cogit.tagGroupSeparator") ?? null));

  $effect(() => {
    void repoSettings(repo)
      .then((settings) => {
        read = settings;
        drafts = new Map(initialDrafts(settings));
      })
      .catch((err) => {
        errors.report(err, "Could not read the repository settings");
        onclose();
      });
  });

  function draftOf(field: SettingField): string | null {
    return drafts.get(field.key) ?? null;
  }

  function current(field: SettingField): string {
    return effective(field, draftOf(field), inherited.get(field.key) ?? null);
  }

  function set(field: SettingField, value: string | null) {
    drafts = new Map(drafts).set(field.key, value === null ? null : normalise(field, value));
  }

  async function save() {
    if (!read || saving || problem) return;
    const pending = changes(read, drafts);
    if (pending.length === 0) {
      onclose();
      return;
    }
    saving = true;
    try {
      await writeRepoSettings(repo, pending);
    } catch (err) {
      errors.report(err, "Could not save the repository settings");
      saving = false;
      return;
    }
    await onsaved();
    onclose();
  }
</script>

<Dialog
  title="Repository Settings"
  {onclose}
  onconfirm={() => void save()}
  width="min(760px, 94vw)"
  height="min(600px, 88vh)"
  flush
>
  <div class="panes">
    <div class="tabs" role="tablist" aria-label="Setting groups" aria-orientation="vertical">
      {#each TABS as [id, title] (id)}
        <button
          type="button"
          role="tab"
          class="tab"
          class:active={tab === id}
          aria-selected={tab === id}
          onclick={() => (tab = id)}
        >
          {title}
        </button>
      {/each}
    </div>

    <section class="content" role="tabpanel">
      <p class="lead">
        Edit the Git settings of <strong>{name}</strong>. They are written to this repository's
        .git/config; an option left unset is inherited from your own Git configuration
        (Repository ▸ Edit Git Config ▸ User…).
      </p>

      {#if read === null}
        <p class="muted">Reading the settings…</p>
      {:else}
        {#each shown as field (field.key)}
          <div class="row">
            {#if field.control.kind === "text"}
              <label class="text">
                <span class="label">{field.label}</span>
                <input
                  type="text"
                  value={draftOf(field) ?? ""}
                  placeholder={draftOf(field) === "" && field.empty
                    ? field.empty
                    : (inherited.get(field.key) ?? field.control.placeholder ?? field.fallback)}
                  oninput={(event) => set(field, event.currentTarget.value)}
                />
              </label>
            {:else if field.control.kind === "bool"}
              {@const control = field.control}
              <Checkbox
                checked={isOn(current(field), control)}
                label={field.label}
                onchange={(checked) => set(field, checked ? control.on : control.off)}
              />
            {:else}
              <fieldset>
                <legend class="label">{field.label}</legend>
                {#each field.control.options as [value, title] (value)}
                  <Radio name={field.key} checked={current(field) === value} onchange={() => set(field, value)} label={title} />
                {/each}
              </fieldset>
            {/if}

            <div class="meta">
              <span>{origin(field, draftOf(field), inherited.get(field.key) ?? null)}</span>
              {#if draftOf(field) !== null}
                <button type="button" class="inherit" onclick={() => set(field, null)}>Use inherited</button>
              {/if}
            </div>
            {#if field.key === "cogit.tagGroupSeparator" && problem}
              <p class="problem" role="alert">{problem}</p>
            {/if}
            {#if field.hint}<p class="hint">{field.hint}</p>{/if}
            <p class="where">Stored as {storage(field)}.</p>
          </div>
        {/each}

        {#if tab === "encoding"}
          <div class="row">
            <span class="label">File contents</span>
            <p class="hint">
              Shown as UTF-8; a file in another encoding is flagged in the Diff panel. Not configurable
              yet, so nothing is stored.
            </p>
          </div>
        {/if}
      {/if}
    </section>
  </div>

  {#snippet footer()}
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button
      type="button"
      class="btn primary"
      disabled={read === null || saving || problem !== null}
      onclick={() => void save()}
    >
      Save
    </button>
  {/snippet}
</Dialog>

<style>
  .panes {
    display: flex;
    flex: 1 1 auto;
    min-height: 0;
  }

  .tabs {
    display: flex;
    flex-direction: column;
    flex: 0 0 auto;
    min-width: 150px;
    padding: var(--sp-3) 0;
    border-right: 1px solid var(--divider);
  }

  .tab {
    padding: var(--sp-3) var(--sp-5);
    background: none;
    border: 0;
    color: var(--text-primary);
    font: inherit;
    font-size: var(--fs-dense);
    text-align: left;
    white-space: nowrap;
    cursor: default;
  }

  .tab:hover {
    background: var(--state-hover);
  }

  .tab.active {
    background: var(--state-selected);
    box-shadow: inset 2px 0 0 var(--status-ref);
  }

  .content {
    flex: 1 1 auto;
    min-width: 0;
    padding: var(--sp-5) var(--sp-6);
    overflow: auto;
    font-size: var(--fs-dense);
  }

  .lead {
    margin: 0 0 var(--sp-6);
    color: var(--text-secondary);
    line-height: 1.45;
  }

  .row {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    margin-bottom: var(--sp-6);
    overflow-wrap: anywhere;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .label {
    font-weight: 600;
  }

  fieldset {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    margin: 0;
    padding: 0;
    border: 0;
  }

  legend {
    padding: 0;
    margin-bottom: var(--sp-2);
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--sp-4);
    color: var(--text-secondary);
  }

  .inherit {
    padding: 0;
    background: none;
    border: 0;
    color: var(--status-ref);
    font: inherit;
    text-decoration: underline;
    cursor: pointer;
  }

  .hint,
  .where,
  .muted {
    margin: 0;
    color: var(--text-secondary);
    line-height: 1.4;
  }

  .where {
    font-style: italic;
  }

  .problem {
    margin: 0;
    color: var(--status-modify);
  }
</style>
