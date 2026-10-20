<script lang="ts">
  import { syntaxHighlighting } from "@codemirror/language";
  import { EditorState, StateEffect } from "@codemirror/state";
  import { EditorView, lineNumbers } from "@codemirror/view";
  import { classHighlighter } from "@lezer/highlight";
  import EmptyState from "$components/common/EmptyState.svelte";
  import { contentNotice, SHOW_ANYWAY_LIMIT } from "$lib/file-content";
  import { fileContent, type DiffSpec, type FileContent, type RepoId } from "$lib/ipc";
  import { languageExtension, styleNonce, themeSpec } from "$lib/solver-editor";
  import { errors } from "$stores/errors.svelte";

  /** An unchanged or ignored file shown whole, read-only, highlighted as the diff is. */
  interface Props {
    repo: RepoId;
    spec: DiffSpec;
    path: string;
  }

  let { repo, spec, path }: Props = $props();

  let content = $state.raw<FileContent | null>(null);
  let limit = $state<number | null>(null);
  let host: HTMLDivElement | undefined = $state();

  $effect(() => {
    const asked = { repo, spec, path, limit };
    let current = true;
    fileContent(asked.repo, asked.spec, asked.path, asked.limit).then(
      (read) => {
        if (current) content = read;
      },
      (err) => {
        if (current) errors.report(err, "Could not read the file", asked.path);
      },
    );
    return () => {
      current = false;
    };
  });

  // Another file starts at the text limit again.
  $effect.pre(() => {
    void path;
    limit = null;
  });

  $effect(() => {
    const read = content;
    if (!host || read?.kind !== "text") return;
    const view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: read.text,
        extensions: [
          styleNonce(),
          EditorView.theme(themeSpec),
          lineNumbers(),
          syntaxHighlighting(classHighlighter),
          EditorState.readOnly.of(true),
          EditorView.contentAttributes.of({ "aria-label": `${path}, read-only` }),
        ],
      }),
    });
    let alive = true;
    void languageExtension(path).then((grammar) => {
      if (alive && grammar) view.dispatch({ effects: StateEffect.appendConfig.of(grammar) });
    });
    return () => {
      alive = false;
      view.destroy();
    };
  });

  const notice = $derived(content && content.kind !== "text" ? contentNotice(content) : null);
</script>

{#if content?.kind === "text"}
  <div class="file" data-select-text="diff" bind:this={host}></div>
{:else if notice}
  <EmptyState
    title={notice}
    action={content?.kind === "tooLarge" ? "Show anyway" : undefined}
    onaction={() => (limit = SHOW_ANYWAY_LIMIT)}
  />
{/if}

<style>
  .file {
    flex: 1 1 auto;
    min-height: 0;
    height: 100%;
    overflow: hidden;
    background: var(--bg-editor);
  }

  .file :global(.cm-editor) {
    height: 100%;
  }
</style>
