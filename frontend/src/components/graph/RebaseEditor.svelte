<script lang="ts">
  import { untrack } from "svelte";
  import { shortOid } from "$lib/format";
  import { moveEntry, planChanged, planProblem, previewCount } from "$lib/rebase-plan";
  import { pointerDrag } from "$lib/pointer-drag";
  import Checkbox from "$components/common/Checkbox.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import Select from "$components/common/Select.svelte";
  import type { TodoAction, TodoEntry } from "$lib/ipc";

  interface Props {
    base: string;
    plan: TodoEntry[];
    published: boolean;
    busy: boolean;
    onplan: (plan: TodoEntry[]) => void;
    paused: boolean;
    onpaused: (paused: boolean) => void;
    onrun: () => void;
    onclose: () => void;
  }

  let { base, plan, published, busy, onplan, paused, onpaused, onrun, onclose }: Props =
    $props();

  const ACTIONS: TodoAction[] = ["pick", "reword", "edit", "squash", "fixup", "drop"];

  /** The plan as git wrote it, to tell a changed one from it. */
  const initial = untrack(() => plan);

  const problem = $derived(planProblem(plan));
  const remaining = $derived(previewCount(plan));
  const runnable = $derived(problem === null && !busy);

  /** The row an entry is being dragged over (R-450). */
  let over = $state<number | null>(null);
  const entryDrag = {
    onover: (target: string | null) => (over = target === null ? null : Number(target)),
    ondrop: (source: string, target: string) => onplan(moveEntry(plan, Number(source), Number(target))),
  };

  function setAction(index: number, action: TodoAction) {
    onplan(plan.map((entry, at) => (at === index ? { ...entry, action } : entry)));
  }

  function setMessage(index: number, message: string) {
    onplan(plan.map((entry, at) => (at === index ? { ...entry, message } : entry)));
  }

  function nudge(index: number, delta: number) {
    onplan(moveEntry(plan, index, index + delta));
  }

  function run() {
    if (runnable) onrun();
  }
</script>

<Dialog
  title="Rebase {plan.length} commits onto {shortOid(base)}"
  {onclose}
  onconfirm={run}
  dirty={planChanged(initial, plan)}
  width="min(720px, 92vw)"
  flush
>
  {#if published}
    <p class="danger">
      Some of these commits are already on a remote. Rebasing gives them new ids, so the
      branch will need a force-push and anyone who pulled it will have to reset.
    </p>
  {/if}

  <div class="list" use:pointerDrag={entryDrag}>
    {#each plan as entry, index (entry.oid)}
      <div
        class="row"
        class:dropped={entry.action === "drop"}
        class:over={over === index}
        data-drag={index}
        data-drop={index}
      >
        <span class="grip" aria-hidden="true">⠿</span>

        <span class="action">
          <Select
            value={entry.action}
            label="Action for {shortOid(entry.oid)}"
            options={ACTIONS.map((action) => [action, action] as const)}
            onchange={(action) => setAction(index, action)}
          />
        </span>

        <span class="oid">{shortOid(entry.oid)}</span>

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

  {#snippet footer()}
    <span class="preview">{remaining} commits will remain</span>
    <span class="pause">
      <Checkbox checked={paused} onchange={(checked) => onpaused(checked)} label="Pause after each commit" />
    </span>
    <span class="problem">{problem ?? ""}</span>
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="button" class="btn primary" disabled={!runnable} onclick={run}>
      {busy ? "Rebasing…" : "Start Rebase"}
    </button>
  {/snippet}
</Dialog>

<style>
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

  .action {
    display: flex;
    flex: 0 0 92px;
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

  /* The shared dialog sizes text fields for a form row; here the field shares its row. */
  .list .row input[type="text"] {
    flex: 1 1 auto;
    width: auto;
    min-width: 0;
    height: 22px;
    padding: 0 var(--sp-3);
  }

  .preview {
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }

  .pause {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    color: var(--text-secondary);
    font-size: var(--fs-header);
  }

  .problem {
    flex: 1 1 auto;
    color: var(--status-modify);
    font-size: var(--fs-header);
  }

  .nudge {
    flex: 0 0 auto;
    height: var(--h-button-sm);
    padding: 0 var(--sp-3);
    background: var(--surface-raised);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-header);
    cursor: default;
  }

  .nudge:hover:not(:disabled) {
    background: var(--state-hover);
  }

  .nudge:disabled {
    opacity: 0.4;
  }
</style>
