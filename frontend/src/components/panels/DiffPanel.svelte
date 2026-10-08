<script lang="ts">
  import Button from "$components/common/Button.svelte";
  import ConflictView from "$components/diff/ConflictView.svelte";
  import MergeView from "$components/diff/MergeView.svelte";
  import DiffView from "$components/diff/DiffView.svelte";
  import DiffEditPane from "$components/diff/DiffEditPane.svelte";
  import { editOffer } from "$lib/diff-edit";
  import { diffEdit, type CaretAt } from "$stores/diff-edit.svelte";
  import { untrack } from "svelte";
  import ImageDiff from "$components/diff/ImageDiff.svelte";
  import SubmoduleDiff from "$components/diff/SubmoduleDiff.svelte";
  import { openEditorWindow, type ConflictSide, type Whitespace } from "$lib/ipc";
  import { errors } from "$stores/errors.svelte";
  import { conflicts } from "$stores/conflicts.svelte";
  import { diff } from "$stores/diff.svelte";
  import type { Snippet } from "svelte";

  /** Everything this panel shows comes from its own stores; the callbacks are the actions
      that reach past it — staging touches the index, blame opens a window of its own. */
  interface Props {
    onstage: (selected: ReadonlySet<string>, reverse: boolean) => void;
    onblame: () => void;
    onwhitespace: (mode: Whitespace) => void;
    onexpand: (whole: boolean) => void;
    onresolve: (side: ConflictSide) => void;
    onpopoutmerge: () => void;
    onresolveText: (text: string) => void;
    /** `git submodule update --init` for the path the panel is showing. */
    oninitsubmodule: (path: string) => void;
    /** Shown when there is nothing to diff: commit details, or why there is nothing. */
    fallback: Snippet;
    /** The Diff panel has the focus, so the keys of the diff are its (11 §7). */
    active: boolean;
  }

  let {
    oninitsubmodule,
    onstage,
    onblame,
    onwhitespace,
    onexpand,
    onresolve,
    onpopoutmerge,
    onresolveText,
    fallback,
    active,
  }: Props = $props();

  const offer = $derived(diff.diff && diff.shownSpec ? editOffer(diff.shownSpec, diff.diff) : null);

  // Another file asks before it is shown (`diff.load`); a diff gone for other reasons (another
  // repository, the panel cleared) takes the editor with it, its edits saved or dropped.
  $effect(() => {
    const path = diff.path;
    const repo = diff.repo;
    untrack(() => {
      const opened = diffEdit.opened;
      const moved = diffEdit.target !== null && (diffEdit.target !== path || (opened !== null && opened.repo !== repo));
      if (moved) void diffEdit.leave(false);
    });
  });

  /** The Edit button: the file in an editor window of its own (F-722). */
  function editWindow() {
    const repo = diff.repo;
    const path = diff.shownPath;
    if (repo !== null && path) void openEditorWindow(repo, path).catch((err) => errors.report(err, "Could not open the editor"));
  }

  /** A click in the right pane of a working-tree diff: the caret goes where it was. */
  function startEdit(at?: CaretAt) {
    const repo = diff.repo;
    const spec = diff.shownSpec;
    const path = diff.shownPath;
    if (repo !== null && spec && path) void diffEdit.start(repo, spec, path, "diff", at ?? null);
  }
</script>

{#if conflicts.path && conflicts.autoResolved(conflicts.path)}
  {@const shown = conflicts.path}
  <p class="rerere" role="status">
    rerere resolved this file as the same conflict was resolved before. Check it, then mark it resolved.
    <Button onclick={() => conflicts.forget(shown)}>Forget Resolution</Button>
  </p>
{/if}
{#if conflicts.path && conflicts.regions.length > 0}
  <MergeView
    path={conflicts.path}
    regions={conflicts.regions}
    onresolve={(side) => onresolve(side)}
    onsolver={onpopoutmerge}
    oncancel={() => conflicts.close()}
    {active}
  />
{:else if conflicts.path}
  <ConflictView
    path={conflicts.path}
    base={conflicts.base}
    ours={conflicts.ours}
    theirs={conflicts.theirs}
    binary={conflicts.binary}
    kind={conflicts.kind}
    tooLarge={conflicts.tooLarge}
    missingOurs={conflicts.missingOurs}
    missingTheirs={conflicts.missingTheirs}
    onresolve={(side) => onresolve(side)}
    onresolveText={(text) => onresolveText(text)}
    onsolver={onpopoutmerge}
    onunsaved={(unsaved) => conflicts.markUnsaved(unsaved)}
  />
{:else if diff.gone && diff.path}
  <p class="gone detail">File deleted: {diff.path}</p>
{:else if diff.error && diff.path}
  <p class="error detail">{diff.error.message}</p>
{:else if diff.diff?.kind === "submodule" && diff.shownPath}
  <SubmoduleDiff
    path={diff.shownPath}
    recorded={diff.diff.recorded}
    previous={diff.diff.previous}
    checkedOut={diff.diff.checkedOut}
    inIndex={diff.diff.inIndex}
    oninit={() => oninitsubmodule(diff.shownPath ?? "")}
  />
{:else if diff.diff?.kind === "image"}
  <ImageDiff
    before={diff.images[0]}
    after={diff.images[1]}
    oldSize={diff.diff.oldSize}
    newSize={diff.diff.newSize}
    mime={diff.diff.mime}
  />
{:else if diffEdit.isFor(diff.shownPath)}
  <DiffEditPane ondone={() => void diffEdit.leave()} />
{:else if diff.diff && diff.shownPath}
  <DiffView
    edit={offer}
    onedit={startEdit}
    oneditwindow={editWindow}
    diff={diff.diff}
    path={diff.shownPath}
    stageable={diff.stageable}
    onstage={(selected, reverse) => onstage(selected, reverse)}
    {onblame}
    whitespace={diff.whitespace}
    onwhitespace={(mode) => onwhitespace(mode)}
    onexpand={(whole) => onexpand(whole)}
    {active}
  />
{:else}
  {@render fallback()}
{/if}

<style>
  .error {
    color: var(--status-delete);
    user-select: text;
  }

  .gone {
    color: var(--text-secondary);
  }

  .detail {
    padding: var(--sp-5);
    font-size: var(--fs-dense);
  }

  .rerere {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    margin: 0;
    padding: var(--sp-2) var(--sp-4);
    font-size: var(--fs-dense);
    border-bottom: 1px solid var(--c-border);
  }
</style>
