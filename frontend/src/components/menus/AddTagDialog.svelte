<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import { tagNameHint } from "$lib/tag-dialog";

  /** #6: name, annotation and the commit the tag lands on, visible before it is made. */
  interface Props {
    oid: string;
    subject: string;
    taken: readonly string[];
    /** `git check-ref-format` and the taken check, asked once on submit. */
    check: (name: string) => Promise<string | null>;
    onadd: (name: string, message: string) => Promise<void>;
    onclose: () => void;
  }

  let { oid, subject, taken, check, onadd, onclose }: Props = $props();

  let name = $state("");
  let message = $state("");
  let touched = $state(false);
  let refused = $state<{ name: string; problem: string } | null>(null);
  let busy = $state(false);
  let field: HTMLInputElement | undefined = $state();

  const hint = $derived(tagNameHint(name, taken));
  const problem = $derived(refused?.name === name ? refused.problem : hint);
  const kind = $derived(message.trim() === "" ? "lightweight" : "annotated");

  async function submit() {
    touched = true;
    if (hint !== null || busy) return;
    busy = true;
    try {
      const said = await check(name);
      if (said !== null) {
        refused = { name, problem: said };
        return;
      }
      await onadd(name, message);
    } finally {
      busy = false;
    }
  }

  $effect(() => {
    field?.focus();
  });
</script>

<Dialog title="Add Tag" {onclose} onconfirm={() => void submit()} width="min(520px, 92vw)">
  <div class="form">
    <div class="target">
      <span class="caption">Commit</span>
      <span class="commit truncate" title="{oid} {subject}">
        <span class="mono">{oid.slice(0, 7)}</span>
        <span class="subject truncate">{subject}</span>
      </span>
    </div>

    <label class="field">
      <span class="caption">Name</span>
      <input
        bind:this={field}
        type="text"
        bind:value={name}
        oninput={() => (touched = true)}
        placeholder="v1.0"
        aria-invalid={touched && problem !== null}
      />
      {#if touched && problem}<span class="problem" role="alert">{problem}</span>{/if}
    </label>

    <label class="field">
      <span class="caption">Message</span>
      <textarea bind:value={message} rows="5" placeholder="Leave empty for a lightweight tag"></textarea>
      <span class="hint">
        {kind === "annotated"
          ? "An annotated tag: the message, your name and the date are stored with it."
          : "A lightweight tag: only a name pointing at the commit."}
      </span>
    </label>
  </div>

  {#snippet footer()}
    <span class="grow"></span>
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    <button type="button" class="btn primary" disabled={hint !== null || busy} onclick={() => void submit()}>
      {busy ? "Adding…" : "Add Tag"}
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

  .field,
  .target {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
  }

  .caption {
    color: var(--text-secondary);
  }

  .commit {
    display: flex;
    gap: var(--sp-3);
    min-width: 0;
  }

  .subject {
    min-width: 0;
  }

  textarea {
    width: 100%;
    min-height: 88px;
    padding: var(--sp-3) var(--sp-4);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font: inherit;
    font-size: var(--fs-dense);
    resize: vertical;
  }

  .problem {
    color: var(--status-delete);
  }

  .hint {
    color: var(--text-secondary);
  }

  .grow {
    flex: 1 1 auto;
  }
</style>
