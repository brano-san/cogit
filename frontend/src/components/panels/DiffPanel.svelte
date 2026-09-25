<script lang="ts">
  import ConflictView from "$components/diff/ConflictView.svelte";
  import MergeView from "$components/diff/MergeView.svelte";
  import DiffView from "$components/diff/DiffView.svelte";
  import ImageDiff from "$components/diff/ImageDiff.svelte";
  import SubmoduleDiff from "$components/diff/SubmoduleDiff.svelte";
  import type { ConflictSide, Whitespace } from "$lib/ipc";
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
  }: Props = $props();
</script>

{#if conflicts.path && conflicts.regions.length > 0}
  <MergeView
    path={conflicts.path}
    regions={conflicts.regions}
    onsave={(text) => onresolveText(text)}
    oncancel={() => conflicts.close()}
    onpopout={onpopoutmerge}
  />
{:else if conflicts.path}
  <ConflictView
    path={conflicts.path}
    base={conflicts.base}
    ours={conflicts.ours}
    theirs={conflicts.theirs}
    binary={conflicts.binary}
    onresolve={(side) => onresolve(side)}
    onresolveText={(text) => onresolveText(text)}
  />
{:else if diff.error && diff.path}
  <p class="error detail">{diff.error.message}</p>
{:else if diff.diff?.kind === "submodule" && diff.shownPath}
  <SubmoduleDiff
    path={diff.shownPath}
    recorded={diff.diff.recorded}
    previous={diff.diff.previous}
    checkedOut={diff.diff.checkedOut}
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
{:else if diff.diff && diff.shownPath}
  <DiffView
    diff={diff.diff}
    path={diff.shownPath}
    stageable={diff.stageable}
    onstage={(selected, reverse) => onstage(selected, reverse)}
    {onblame}
    whitespace={diff.whitespace}
    onwhitespace={(mode) => onwhitespace(mode)}
    onexpand={(whole) => onexpand(whole)}
  />
{:else}
  {@render fallback()}
{/if}

<style>
  .error {
    color: var(--status-delete);
    user-select: text;
  }

  .detail {
    padding: var(--sp-5);
    font-size: var(--fs-dense);
  }
</style>
