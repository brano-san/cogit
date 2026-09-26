<script lang="ts">
  import Checkbox from "$components/common/Checkbox.svelte";
  import Radio from "$components/common/Radio.svelte";
  import {
    COLUMN_LABELS,
    moveColumn,
    reconcileRows,
    toggleColumn,
    visibleColumns,
    type ColumnRow,
  } from "$lib/graph-columns";
  import { blockedModes, GRAPH_MODES, type GraphMode } from "$lib/graph-mode-conflicts";
  import { FIELD_LABELS, FILTER_FIELDS, toggled } from "$lib/filter-fields";
  import { forget } from "$lib/filter-patterns";
  import { fieldDisabled, type Field } from "$lib/preferences";
  import { LONG_LINK_ROWS_MAX, type Settings } from "$lib/settings";
  import { pointerDrag } from "$lib/pointer-drag";

  /** One graph setting of Preferences ▸ Graph & History (#23). Applied as it changes. */
  interface Props {
    field: Field;
    value: Settings;
    onset: <K extends keyof Settings>(key: K, next: Settings[K]) => void;
  }

  let { field, value, onset }: Props = $props();

  const TIME_FORMATS = [
    ["relative", "3 days ago"],
    ["date", "yesterday · Tuesday · 09-09-26"],
    ["dateTime", "yesterday 14:05 · 09-09-26 14:05"],
  ] as const;

  const DENSITIES = [
    ["compact", "Compact"],
    ["normal", "Normal"],
    ["comfortable", "Comfortable"],
  ] as const;

  const disabled = $derived(fieldDisabled(value, field));
  const blocked = $derived(blockedModes(value).find((entry) => entry.mode === field.key));

  /** Where the hidden columns sit, as last arranged here; the setting only keeps the rest. */
  let arranged = $state.raw<ColumnRow[]>([]);
  const rows = $derived(reconcileRows(arranged, value.graphColumns));
  /** The row a column is being dragged over (R-450). */
  let over = $state<number | null>(null);
  const columnDrag = {
    onover: (target: string | null) => (over = target === null ? null : Number(target)),
    ondrop: (source: string, target: string) => arrange(moveColumn(rows, Number(source), Number(target))),
  };

  function arrange(next: ColumnRow[]) {
    arranged = next;
    onset("graphColumns", visibleColumns(next));
  }

  function nudge(index: number, delta: number, focus = false) {
    arrange(moveColumn(rows, index, index + delta));
    if (!focus) return;
    const target = Math.min(Math.max(index + delta, 0), rows.length - 1);
    queueMicrotask(() =>
      document.querySelector<HTMLElement>(`[data-column-row="${target}"]`)?.focus(),
    );
  }

  function onrowkey(event: KeyboardEvent, index: number) {
    if (!event.altKey || (event.key !== "ArrowUp" && event.key !== "ArrowDown")) return;
    event.preventDefault();
    nudge(index, event.key === "ArrowUp" ? -1 : 1, true);
  }

  const avatarsOff = $derived(value.avatars !== "gravatar");
</script>

{#if field.key === "graphColumns"}
  <div class="row choice">
    <span>{field.label}</span>
    <div class="columns" role="list" aria-label="Graph columns, in order" use:pointerDrag={columnDrag}>
      {#each rows as row, index (row.id)}
        <div
          class="column"
          role="listitem"
          class:over={over === index}
          data-drag={index}
          data-drop={index}
        >
          <button
            type="button"
            class="grip"
            data-column-row={index}
            aria-label="Move {COLUMN_LABELS[row.id]}: Alt+Up or Alt+Down, position {index + 1} of {rows.length}"
            onkeydown={(event) => onrowkey(event, index)}>⠿</button
          >
          <Checkbox
            checked={row.shown}
            label={COLUMN_LABELS[row.id]}
            disabled={row.id === "avatar" && avatarsOff}
            onchange={() => arrange(toggleColumn(rows, row.id))}
          />
          {#if row.id === "avatar" && avatarsOff}
            <span class="why">Avatars are off under Authors below.</span>
          {/if}
          <span class="grow"></span>
          <button
            type="button"
            class="nudge"
            aria-label="Move {COLUMN_LABELS[row.id]} up"
            disabled={index === 0}
            onclick={() => nudge(index, -1)}>↑</button
          >
          <button
            type="button"
            class="nudge"
            aria-label="Move {COLUMN_LABELS[row.id]} down"
            disabled={index === rows.length - 1}
            onclick={() => nudge(index, 1)}>↓</button
          >
        </div>
      {/each}
    </div>
  </div>
{:else if field.key === "graphTimeFormat"}
  <div class="row choice nested" role="radiogroup" aria-label={field.label} aria-disabled={disabled}>
    <span class:off={disabled}>{field.label}</span>
    <div class="options">
      {#each TIME_FORMATS as [id, example] (id)}
        <span class:off={disabled}>
          <Radio name="graphTimeFormat" checked={value.graphTimeFormat === id} {disabled} onchange={() => onset("graphTimeFormat", id)}>
            <span class="mono">{example}</span>
          </Radio>
        </span>
      {/each}
      {#if disabled}<span class="why">The Time column is hidden.</span>{/if}
    </div>
  </div>
{:else if field.key === "graphDensity"}
  <div class="row choice" role="radiogroup" aria-label={field.label}>
    <span>{field.label}</span>
    <div class="options">
      {#each DENSITIES as [id, title] (id)}
        <Radio name="graphDensity" checked={value.graphDensity === id} onchange={() => onset("graphDensity", id)} label={title} />
      {/each}
    </div>
  </div>
{:else if field.key === "graphFilterFields"}
  <div class="row choice">
    <span>{field.label}</span>
    <div class="options" role="group" aria-label={field.label}>
      {#each FILTER_FIELDS as each (each)}
        <span title={FIELD_LABELS[each].title}>
          <Checkbox
            checked={value.graphFilterFields.includes(each)}
            label={FIELD_LABELS[each].label}
            onchange={() => onset("graphFilterFields", toggled(value.graphFilterFields, each))}
          />
        </span>
      {/each}
    </div>
  </div>
{:else if field.key === "graphFilterPatterns"}
  <div class="row choice">
    <span>{field.label}</span>
    {#if value.graphFilterPatterns.length === 0}
      <span class="why">None yet: Remember Pattern in the magnifier's menu keeps the filter in the field.</span>
    {:else}
      <ul class="patterns" aria-label={field.label}>
        {#each value.graphFilterPatterns as pattern (pattern)}
          <li>
            <span class="pattern truncate" title={pattern}>{pattern}</span>
            <button
              type="button"
              class="forget"
              title="Forget this filter"
              aria-label="Forget {pattern}"
              onclick={() => onset("graphFilterPatterns", forget(value.graphFilterPatterns, pattern))}>✕</button
            >
          </li>
        {/each}
      </ul>
    {/if}
  </div>
{:else if field.key === "graphLongLinkRows"}
  <label class="row">
    <span>{field.label}</span>
    <span class="slider">
      <input
        type="range"
        min="0"
        max="200"
        value={Math.min(value.graphLongLinkRows, 200)}
        oninput={(e) => onset("graphLongLinkRows", Math.min(e.currentTarget.valueAsNumber, LONG_LINK_ROWS_MAX))}
      />
      <output>{value.graphLongLinkRows === 0 ? "never" : `${value.graphLongLinkRows} rows`}</output>
    </span>
  </label>
{:else if field.key === "graphStripes" || field.key === "graphHighlightChecked" || (GRAPH_MODES as readonly string[]).includes(field.key)}
  {@const key = field.key as "graphStripes" | "graphHighlightChecked" | GraphMode}
  <div class="row check">
    <Checkbox
      checked={value[key]}
      label={field.label}
      disabled={blocked !== undefined}
      onchange={(next) => onset(key, next)}
    />
    {#if blocked}<span class="why">{blocked.reason}</span>{/if}
  </div>
{/if}

<style>
  .row {
    display: grid;
    grid-template-columns: 200px minmax(0, 1fr);
    align-items: center;
    justify-items: start;
    gap: var(--sp-4);
    min-height: var(--h-row);
    margin-bottom: var(--sp-3);
    font-size: var(--fs-dense);
  }

  .row.choice {
    align-items: start;
  }

  .row.check {
    grid-template-columns: auto minmax(0, 1fr);
    gap: var(--sp-3);
  }

  .row.nested > span:first-child {
    padding-left: var(--sp-7);
  }

  .columns {
    justify-self: stretch;
    margin: 0;
    padding: var(--sp-1) 0;
    list-style: none;
    background: var(--surface-input);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
  }

  .column {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    height: var(--h-row);
    padding: 0 var(--sp-3);
    cursor: default;
  }

  .column:hover {
    background: var(--state-hover);
  }

  .column.over {
    box-shadow: inset 0 1px 0 var(--status-ref);
  }

  .grip {
    padding: 0;
    background: none;
    border: 0;
    color: var(--text-secondary);
    font: inherit;
    cursor: grab;
  }

  .grip:focus-visible {
    outline: 1px solid var(--status-ref);
    outline-offset: 1px;
  }

  .grow {
    flex: 1 1 auto;
  }

  .nudge {
    width: 20px;
    height: 20px;
    padding: 0;
    background: none;
    border: 0;
    border-radius: var(--r-sm);
    color: var(--text-secondary);
    font: inherit;
  }

  .nudge:hover:not(:disabled) {
    background: var(--state-hover);
    color: var(--text-primary);
  }

  .nudge:disabled {
    opacity: 0.4;
  }

  .options {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .patterns {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    justify-self: stretch;
    min-width: 0;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .patterns li {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-width: 0;
  }

  .pattern {
    flex: 1 1 auto;
    min-width: 0;
    font-family: var(--font-mono);
  }

  .forget {
    flex: 0 0 auto;
    padding: 0 var(--sp-2);
    background: none;
    color: var(--text-secondary);
    border: 0;
    cursor: default;
  }

  .forget:hover {
    color: var(--text-primary);
  }

  .slider {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    justify-self: stretch;
  }

  .why {
    color: var(--text-secondary);
    font-size: 11px;
  }

  .off {
    color: var(--text-secondary);
    opacity: 0.6;
  }
</style>
