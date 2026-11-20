<script lang="ts">
  import Button from "$components/common/Button.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import Select from "$components/common/Select.svelte";
  import { AI_PROVIDERS, aiConfig, diffBytes, MAX_DIFF_BYTES, MIN_DIFF_BYTES, validBaseUrl } from "$lib/ai-config";
  import { removeAiKey, setAiKey, testAiProvider, type AiConfig, type AiProvider } from "$lib/ipc";
  import { aiSettings } from "$stores/ai-settings.svelte";
  import { confirmation } from "$stores/confirm.svelte";
  import { masterPassword } from "$stores/master-password.svelte";
  import { notices } from "$stores/notices.svelte";

  let { config, onchange }: { config: AiConfig | null; onchange: (value: AiConfig | null) => void } = $props();

  const providers = [
    ["off", "Off"],
    ["anthropic", AI_PROVIDERS.anthropic.label],
    ["openai", AI_PROVIDERS.openai.label],
  ] as const;

  let base = $state("");
  let keyDraft = $state("");
  let editing = $state(false);
  let changing = $state(false);
  let testing = $state(false);
  let tested = $state<string | null>(null);

  const key = $derived(aiSettings.at(config));
  const validUrl = $derived(validBaseUrl(base.trim()));
  const serviceReady = $derived(config !== null && validUrl && base.trim() === config.baseUrl && key !== null);
  const canTest = $derived(serviceReady && key?.state !== "locked" && (!key?.required || key.stored));
  const signature = $derived(config ? JSON.stringify(config) : "");

  $effect(() => { base = config?.baseUrl ?? ""; });

  function select(provider: AiProvider | "off"): void {
    if (provider === "off") {
      onchange(null);
    } else {
      onchange({ ...aiConfig(provider), instructions: config?.instructions ?? "", maxDiffBytes: config?.maxDiffBytes ?? 40_000 });
    }
    tested = null;
  }

  async function editKey(): Promise<void> {
    if (!serviceReady || changing) return;
    try {
      if (await masterPassword.ready()) {
        keyDraft = "";
        editing = true;
      }
    } catch (err) {
      notices.report(err, "Could not open the password store");
    }
  }

  function closeKey(): void {
    if (changing) return;
    keyDraft = "";
    editing = false;
  }

  async function saveKey(): Promise<void> {
    if (!config || !serviceReady || changing || !keyDraft.trim()) return;
    const target = config;
    const secret = keyDraft;
    keyDraft = "";
    changing = true;
    try {
      await setAiKey({ config: target, key: secret });
      editing = false;
      tested = null;
    } catch (err) {
      notices.report(err, "Could not save the AI key");
    } finally {
      changing = false;
    }
    await aiSettings.refresh(config);
  }

  async function removeKey(): Promise<void> {
    if (!config || !key?.stored || changing) return;
    const target = config;
    const go = await confirmation.ask({
      title: "Remove API Key",
      message: `Remove the stored API key for ${key.host}?`,
      confirm: "Remove",
      warning: true,
    });
    if (!go) return;
    changing = true;
    try {
      await removeAiKey(target);
      tested = null;
    } catch (err) {
      notices.report(err, "Could not remove the AI key");
    } finally {
      changing = false;
    }
    await aiSettings.refresh(config);
  }

  async function test(): Promise<void> {
    if (!config || !canTest || testing) return;
    const checked = signature;
    tested = null;
    testing = true;
    try {
      await testAiProvider(config);
      tested = checked;
    } catch (err) {
      notices.report(err, "AI Provider Test Failed");
    } finally {
      testing = false;
    }
  }
</script>

<div class="ai-page">
  <p class="hint">Generate a commit message from staged changes. API keys are kept in Cogit's encrypted password store.</p>
  <div class="fields">
    <span>Provider</span>
    <Select value={config?.provider ?? "off"} options={providers} label="Provider" onchange={select} disabled={changing || testing} />

    {#if config}
      {@const provider = AI_PROVIDERS[config.provider]}

      <label for="ai-base">Base URL</label>
      <input
        id="ai-base"
        class="mono"
        type="text"
        spellcheck="false"
        autocomplete="off"
        placeholder={provider.baseUrl}
        aria-invalid={!validUrl}
        aria-describedby="ai-base-hint"
        disabled={changing || testing}
        bind:value={base}
        onchange={() => { if (config && validUrl) onchange({ ...config, baseUrl: base.trim() }); }}
      />
      <p class="hint detail" id="ai-base-hint" class:invalid={!validUrl}>
        {#if !validUrl}
          Enter an HTTP or HTTPS URL without credentials, a query, or a fragment.
        {:else if config.provider === "openai"}
          For Azure OpenAI, use its OpenAI v1 base URL. For Ollama or LM Studio, include /v1.
        {:else}
          Leave blank to use Anthropic's API.
        {/if}
      </p>

      <label for="ai-model">Model</label>
      <input
        id="ai-model"
        class="mono"
        type="text"
        spellcheck="false"
        autocomplete="off"
        placeholder={provider.model}
        value={config.model}
        disabled={changing || testing}
        onchange={(event) => { if (config) onchange({ ...config, model: event.currentTarget.value.trim() }); }}
      />

      <label for="ai-instructions">Custom instructions</label>
      <textarea
        id="ai-instructions"
        value={config.instructions}
        rows="4"
        disabled={changing || testing}
        onchange={(event) => { if (config) onchange({ ...config, instructions: event.currentTarget.value }); }}
      ></textarea>

      <label for="ai-budget">Max diff size (bytes)</label>
      <input
        id="ai-budget"
        class="budget mono"
        type="text"
        inputmode="numeric"
        aria-describedby="ai-budget-hint"
        value={config.maxDiffBytes}
        disabled={changing || testing}
        onchange={(event) => {
          if (!config) return;
          const bytes = diffBytes(Number(event.currentTarget.value.trim() || NaN));
          event.currentTarget.value = String(bytes);
          onchange({ ...config, maxDiffBytes: bytes });
        }}
      />
      <p class="hint detail" id="ai-budget-hint">
        {MIN_DIFF_BYTES.toLocaleString("en-US")}–{MAX_DIFF_BYTES.toLocaleString("en-US")} bytes. The file list is always included.
      </p>

      <span>API key</span>
      <div class="actions">
        {#if key?.state === "locked"}
          <span>Password store locked</span>
          <Button disabled={changing || testing} onclick={() => { if (config) void aiSettings.unlock(config); }}>Unlock…</Button>
        {:else if key?.stored}
          <span>Key saved</span>
          <Button disabled={!serviceReady || changing || testing} onclick={editKey}>Replace…</Button>
          <Button disabled={!serviceReady || changing || testing} onclick={removeKey}>Remove…</Button>
        {:else}
          {#if key}<span>{key.required ? "Key required" : "Optional for this server"}</span>{/if}
          <Button disabled={!serviceReady || changing || testing} onclick={editKey}>Add Key…</Button>
        {/if}
      </div>
    {/if}
  </div>
  {#if config}
    <div class="actions">
      <Button disabled={!canTest || testing || changing} onclick={test}>{testing ? "Testing…" : "Test"}</Button>
      {#if tested === signature}<span role="status">Provider and model responded.</span>{/if}
    </div>
  {/if}
</div>

{#if editing}
  <Dialog title={key?.stored ? "Replace API Key" : "Add API Key"} onclose={closeKey} onconfirm={saveKey}>
    <label class="key-field" for="ai-key">
      <span>API key</span>
      <input id="ai-key" type="password" autocomplete="off" spellcheck="false" disabled={changing} data-autofocus bind:value={keyDraft} />
    </label>
    {#snippet footer()}
      <Button size="md" disabled={changing} onclick={closeKey}>Cancel</Button>
      <Button size="md" variant="primary" disabled={changing || !keyDraft.trim()} onclick={saveKey}>Save</Button>
    {/snippet}
  </Dialog>
{/if}

<style>
  .ai-page {
    display: grid;
    gap: var(--sp-5);
  }

  .fields {
    display: grid;
    grid-template-columns: minmax(110px, max-content) minmax(0, 1fr);
    align-items: center;
    gap: var(--sp-3) var(--sp-5);
  }

  .fields > label {
    align-self: start;
    padding-top: var(--sp-2);
  }

  input, textarea {
    width: 100%;
    min-width: 0;
    box-sizing: border-box;
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    padding: var(--sp-2) var(--sp-3);
    font: inherit;
  }

  input {
    height: var(--h-input);
  }

  textarea {
    resize: vertical;
  }

  input:focus-visible, textarea:focus-visible {
    outline: 1px solid var(--state-focus-ring);
    outline-offset: -1px;
  }

  input[aria-invalid="true"] {
    border-color: var(--status-delete);
  }

  .mono {
    font-family: var(--font-mono);
  }

  .budget {
    max-width: 9em;
  }

  .hint {
    margin: 0;
    color: var(--text-secondary);
  }

  .detail {
    grid-column: 2;
    font-size: var(--fs-small);
  }

  .invalid {
    color: var(--status-delete);
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-3);
  }

  .key-field {
    display: grid;
    gap: var(--sp-2);
  }

  @media (max-width: 560px) {
    .fields {
      grid-template-columns: minmax(0, 1fr);
    }

    .detail {
      grid-column: auto;
    }
  }
</style>
