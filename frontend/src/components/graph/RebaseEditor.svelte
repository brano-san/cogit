<script lang="ts">
  import { moveEntry, planProblem, previewCount } from "$lib/rebase-plan";
  import type { TodoAction, TodoEntry } from "$lib/ipc";

  interface Props {
    base: string;
    plan: TodoEntry[];
    published: boolean;
    busy: boolean;
    onplan: (plan: TodoEntry[]) => void;
    onrun: () => void;
    onclose: () => void;
  }

  let { base, plan, published, busy, onplan, onrun, onclose }: Props = $props();

  const ACTIONS: TodoAction[] = ["pick", "reword", "edit", "squash", "fixup", "drop"];

  const problem = $derived(planProblem(plan));
  const remaining = $derived(previewCount(plan));

  let dragging = $state<number | null>(null);

  function setAction(index: number, action: TodoAction) {
    onplan(plan.map((entry, at) => (at === index ? { ...entry, action } : entry)));
  }

  function setMessage(index: number, message: string) {
    onplan(plan.map((entry, at) => (at === index ? { ...entry, message } : entry)));
  }

  function drop(to: number) {
    if (dragging !== null) onplan(moveEntry(plan, dragging, to));
    dragging = null;
  }

  function nudge(index: number, delta: number) {
    onplan(moveEntry(plan, index, index + delta));
  }

  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
    }
  }
</script>

<svelte:window {onkeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onclose}></div>

<div class="dialog" role="dialog" aria-label="Interactive rebase">
  <header>
    <h2>Rebase {plan.length} commits onto {base.slice(0, 7)}</h2>
    <button type="button" class="icon" onclick={onclose} aria-label="Close">✕</button>
  </header>

  {#if published}
    <p class="danger">
      Some of these commits are already on a remote. Rebasing gives them new ids, so the
      branch will need a force-push and anyone who pulled it will have to reset.
    </p>
  {/if}

  <div class="list">
    {#each plan as entry, index (entry.oid)}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="row"
        class:dropped={entry.action === "drop"}
        class:over={dragging !== null && dragging !== index}
        draggable="true"
        ondragstart={() => (dragging = index)}
        ondragover={(event) => event.preventDefault()}
        ondrop={() => drop(index)}
        ondragend={() => (dragging = null)}
      >
        <span class="grip" aria-hidden="true">⠿</span>

        <select
          value={entry.action}
          aria-label="Action for {entry.oid.slice(0, 7)}"
          onchange={(event) => setAction(index, event.currentTarget.value as TodoAction)}
        >
          {#each ACTIONS as action (action)}<option value={action}>{action}</option>{/each}
        </select>

        <span class="oid">{entry.oid.slice(0, 7)}</span>

        {#if entry.action === "reword"}
          <input
            type="text"
            value={entry.message ?? ""}
            aria-label="New message"
            oninput={(event) => setMessage(index, event.currentTarget.value)}
          />
        {:else}
          <span class="subject truncate">{entry.message ?? ""}</span>
        {/if}

        <button
          type="button"
          class="nudge"
          aria-label="Move up"
          disabled={index === 0}
          onclick={() => nudge(index, -1)}>↑</button
        >
        <button
          type="button"
          class="nudge"
          aria-label="Move down"
          disabled={index === plan.length - 1}
          onclick={() => nudge(index, 1)}>↓</button
        >
      </div>
    {/each}
  </div>

  <footer>
    <span class="preview">{remaining} commits will remain</span>
    {#if problem}<span class="problem">{problem}</span>{/if}
    <button type="button" onclick={onclose}>Cancel</button>
    <button type="button" class="primary" disabled={problem !== null || busy} onclick={onrun}>
      {busy ? "Rebasing…" : "Start Rebase"}
    </button>
  </footer>
</div>

<style>
  .backdrop {
    position: absolute;
    inset: 0;
    z-index: 20;
    background: rgb(0 0 0 / 35%);
  }

  .dialog {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 21;
    display: flex;
    flex-direction: column;
    width: min(720px, 92vw);
    max-height: 82vh;
    background: var(--surface-panel);
    border: 1px solid var(--field-border);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-popover);
    overflow: hidden;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: var(--h-toolbar);
    padding: 0 var(--sp-5);
    border-bottom: 1px solid var(--divider);
  }

  h2 {
    margin: 0;
    font-size: var(--fs-ui);
    font-weight: 600;
  }

  .danger {
    margin: 0;
    padding: var(--sp-4) var(--sp-5);
    background: var(--c-deleted-bg);
    font-size: var(--fs-dense);
    border-bottom: 1px solid var(--divider);
  }

  .list {
    flex: 1 1 auto;
    min-height: 0;
    padding: var(--sp-3) 0;
    overflow: auto;
  }

  .row {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    height: var(--h-row);
    padding: 0 var(--sp-5);
    font-size: var(--fs-dense);
  }

  .row:hover {
    background: var(--state-hover);
  }

  .row.over {
    box-shadow: inset 0 -2px 0 var(--status-ref);
  }

  .row.dropped {
    opacity: 0.5;
    text-decoration: line-through;
  }

  .grip {
    flex: 0 0 auto;
    color: var(--text-secondary);
    cursor: grab;
  }

  select {
    flex: 0 0 92px;
    height: 22px;
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-header);
  }

  .oid {
    flex: 0 0 62px;
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: var(--fs-header);
  }

  .subject {
    flex: 1 1 auto;
    min-width: 0;
  }

  input {
    flex: 1 1 auto;
    min-width: 0;
    height: 22px;
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
  }

  footer {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    padding: var(--sp-4) var(--sp-5);
    border-top: 1px solid var(--divider);
  }

  .preview {
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }

  .problem {
    flex: 1 1 auto;
    color: var(--status-modify);
    font-size: var(--fs-header);
  }

  footer > button:first-of-type {
    margin-left: auto;
  }

  button {
    height: var(--h-input);
    padding: 0 var(--sp-5);
    background: var(--surface-raised);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }

  button:hover {
    background: var(--state-hover);
  }

  button.primary {
    background: var(--status-ref);
    color: var(--c-text-inverse);
    border-color: transparent;
  }

  button.nudge {
    flex: 0 0 auto;
    height: 20px;
    padding: 0 var(--sp-3);
    font-size: var(--fs-header);
  }

  button:disabled {
    opacity: 0.4;
  }

  button.icon {
    height: 22px;
    padding: 0 var(--sp-3);
    background: none;
    border: 0;
    color: var(--text-secondary);
  }
</style>
