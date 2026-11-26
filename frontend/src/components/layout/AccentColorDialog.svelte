<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import { THEME_FILES, validValue } from "$lib/theme";

  interface Props {
    value: string;
    onclose: () => void;
    onselect: (color: string) => void;
  }

  let { value, onclose, onselect }: Props = $props();
  // svelte-ignore state_referenced_locally
  let text = $state(value);
  const color = $derived(text.trim().toLowerCase());
  const valid = $derived(validValue("color", color));

  const colors = THEME_FILES.light.tokens;
  const presets = [
    ["Blue", colors.accent],
    ["Teal", THEME_FILES.lightGrey.tokens.accent],
    ["Green", colors["graph.lane.2"]],
    ["Orange", colors["graph.lane.4"]],
    ["Red", colors["status.danger"]],
    ["Pink", colors["graph.lane.3"]],
    ["Violet", colors["graph.lane.1"]],
  ] as const;

  function apply() {
    if (valid) onselect(color);
  }
</script>

<Dialog title="Accent Color" {onclose} onconfirm={apply}>
  <div class="palette" role="group" aria-label="Preset colors">
    {#each presets as [name, swatch] (name)}
      <button type="button" class="btn swatch" aria-label={name} aria-pressed={color === swatch} data-tip={name}
        onclick={() => (text = swatch!)}>
        <span class="sample" style:background={swatch}></span>
      </button>
    {/each}
  </div>
  <label class="custom">
    <span>Hex color</span>
    <input type="text" class="mono" value={text} placeholder="#RRGGBB" spellcheck="false"
      aria-invalid={!valid} aria-describedby={valid ? undefined : "accent-color-error"}
      style:border-color={valid ? undefined : "var(--status-danger)"}
      oninput={(event) => (text = event.currentTarget.value)} />
    <span class="sample" aria-hidden="true" style:background={valid ? color : undefined}></span>
  </label>
  {#if !valid}
    <p id="accent-color-error" role="alert">Enter a hex color in #RRGGBB format.</p>
  {/if}

  {#snippet footer()}
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="button" class="btn primary" disabled={!valid} onclick={apply}>Apply</button>
  {/snippet}
</Dialog>

<style>
  .palette {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
    margin-bottom: var(--sp-4);
  }

  .swatch {
    padding: var(--sp-1);
  }

  .swatch[aria-pressed="true"] {
    border-color: var(--accent);
  }

  .sample {
    display: block;
    flex: 0 0 18px;
    width: 18px;
    height: 18px;
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
  }

  .custom {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--sp-3);
  }

  p {
    margin: var(--sp-2) 0 0;
    color: var(--status-danger);
  }
</style>
