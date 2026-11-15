<script lang="ts">
  import { untrack } from "svelte";
  import Checkbox from "$components/common/Checkbox.svelte";
  import Disclosure from "$components/common/Disclosure.svelte";
  import SkeletonRows from "$components/common/SkeletonRows.svelte";
  import Callout from "$components/common/template/Callout.svelte";
  import OptionRow from "$components/common/template/OptionRow.svelte";
  import TemplateDialog from "$components/common/template/TemplateDialog.svelte";
  import type { RepoId } from "$lib/ipc";
  import {
    sparseFolders,
    sparsePreview,
    sparseState,
    type SparseChange,
    type SparseFolder,
    type SparsePreview,
    type SparseState,
  } from "$lib/ipc/sparse";
  import {
    applyLabel,
    folderCheck,
    folderRows,
    parsePatterns,
    planOf,
    previewLine,
    toggleFolder,
    type SparsePlan,
  } from "$lib/sparse";
  import { errors } from "$stores/errors.svelte";

  /** Repository ▸ Sparse Checkout… (F-955): on or off, the cone folders or the patterns,
      and what the change does to the working tree before it is made. */
  interface Props {
    repo: RepoId;
    onapply: (change: SparseChange) => void;
    onclose: () => void;
  }

  let { repo, onapply, onclose }: Props = $props();

  const SHOWN = 20;
  /** The preview waits for the ticks to settle: each one counts the whole index. */
  const SETTLE_MS = 250;

  let current = $state.raw<SparseState | null>(null);
  let enabled = $state(false);
  let set = $state.raw<string[]>([]);
  let patterns = $state("");
  let read = $state.raw<Map<string, SparseFolder[]>>(new Map());
  let open = $state.raw<Set<string>>(new Set());
  let preview = $state.raw<SparsePreview | null>(null);
  let previewing = $state(false);

  $effect(() => {
    void sparseState(repo)
      .then((found) => {
        current = found;
        enabled = found.enabled;
        set = found.set;
        patterns = found.set.join("\n");
      })
      .catch((err) => {
        errors.report(err, "Could not read the sparse checkout");
        onclose();
      });
    untrack(() => void load(""));
  });

  async function load(dir: string) {
    if (read.has(dir)) return;
    try {
      const folders = await sparseFolders(repo, dir);
      read = new Map(read).set(dir, folders);
    } catch (err) {
      errors.report(err, "Could not read the folders of HEAD");
    }
  }

  function expand(path: string) {
    const next = new Set(open);
    if (next.has(path)) next.delete(path);
    else {
      next.add(path);
      void load(path);
    }
    open = next;
  }

  const cone = $derived(current?.cone ?? true);
  const chosen = $derived(cone ? set : parsePatterns(patterns));
  const plan = $derived<SparsePlan | null>(current ? planOf(current, enabled, chosen) : null);
  const rows = $derived(folderRows(read, open));
  const children = (dir: string) => (read.get(dir) ?? []).map((folder) => folder.path);

  $effect(() => {
    const change = plan ? toChange(plan) : null;
    preview = null;
    if (!change) return;
    previewing = true;
    let live = true;
    const timer = setTimeout(() => {
      sparsePreview(repo, change)
        .then((counted) => {
          if (live) preview = counted;
        })
        .catch((err) => {
          if (live) errors.report(err, "Could not count the files of the change");
        })
        .finally(() => {
          if (live) previewing = false;
        });
    }, SETTLE_MS);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  });

  function toChange(plan: SparsePlan): SparseChange {
    return plan.kind === "disable" ? { kind: "disable" } : { kind: "set", cone: plan.cone, set: plan.set };
  }

  const blocked = $derived(preview?.blocked ?? []);
  const ready = $derived(plan !== null && preview !== null && !previewing && blocked.length === 0);
  const label = $derived(current ? applyLabel(current, plan) : "Apply");

  function submit() {
    if (ready && plan) onapply(toChange(plan));
  }

  function status(found: SparseState): string {
    if (!found.enabled) return "Off: every file of HEAD is in the working tree.";
    if (!found.cone) return `On, non-cone mode: ${found.set.length} patterns.`;
    const n = found.set.length;
    return n === 0
      ? "On, cone mode: only the files at the top of the repository."
      : `On, cone mode: ${n === 1 ? "1 folder" : `${n} folders`} and the files at the top.`;
  }
</script>

<TemplateDialog
  title="Sparse Checkout"
  {onclose}
  fill
  destructive={plan?.kind === "set"}
  actions={[
    {
      label,
      primary: true,
      disabled: !ready,
      tip: plan?.kind === "disable" ? "git sparse-checkout disable" : "git sparse-checkout set --stdin",
      onclick: submit,
    },
  ]}
>
  {#if current === null}
    <SkeletonRows rows={3} />
  {:else}
    <p class="sub">{status(current)}</p>
    <OptionRow
      bind:checked={enabled}
      label="Sparse checkout"
      hint={current.enabled
        ? "Clear to bring every file back (git sparse-checkout disable)"
        : "Keeps only the chosen folders in the working tree, in cone mode"}
    />

    {#if enabled && cone}
      <p class="sub">
        Tick the folders to keep, each with everything under it. Files at the top of the repository are always
        in, and so are the files right inside each folder on the way to a ticked one: that is git's cone rule.
      </p>
      <div class="tree" role="tree" aria-label="Folders of HEAD">
        {#if !read.has("")}
          <SkeletonRows rows={4} />
        {:else if rows.length === 0}
          <p class="sub empty">HEAD has no folders: only the files at the top would stay.</p>
        {/if}
        {#each rows as row (row.path)}
          <div class="row" role="treeitem" aria-selected="false" aria-expanded={row.folders ? row.open : undefined}
            style:padding-left="calc(var(--sp-2) + {row.depth} * var(--tree-step))">
            <Disclosure
              open={row.open}
              empty={!row.folders}
              label={row.open ? `Close ${row.name}` : `Open ${row.name}`}
              onclick={() => expand(row.path)}
            />
            <Checkbox
              wide
              ariaLabel={row.path}
              tri={{
                state: folderCheck(set, row.path),
                toggle: () => {
                  set = toggleFolder(set, row.path, children);
                  return folderCheck(set, row.path);
                },
              }}
            >
              <span class="name truncate" title={row.path}>{row.name}</span>
            </Checkbox>
          </div>
        {/each}
      </div>
    {:else if enabled}
      <Callout kind="warning">
        This repository uses non-cone patterns, which git deprecates: they are slower and git may drop them. One
        pattern per line, as in .gitignore; a file is kept when the last pattern it matches is not negated.
      </Callout>
      <textarea class="patterns" spellcheck="false" wrap="off" aria-label="Sparse checkout patterns" bind:value={patterns}
      ></textarea>
    {/if}

    {#if plan}
      <p class="sub preview" aria-live="polite">
        {#if preview && !previewing}{previewLine(preview)}{:else}Counting the files…{/if}
      </p>
    {/if}

    {#if blocked.length > 0}
      <Callout kind="danger">
        <div>
          These files would leave the working tree with local changes. Commit, stash or discard them first; git
          would leave them behind:
        </div>
        <ul class="blocked">
          {#each blocked.slice(0, SHOWN) as path (path)}
            <li class="mono truncate" title={path}>{path}</li>
          {/each}
          {#if blocked.length > SHOWN}<li>and {blocked.length - SHOWN} more</li>{/if}
        </ul>
      </Callout>
    {/if}
  {/if}
</TemplateDialog>

<style>
  .tree {
    flex: 1 1 auto;
    min-height: calc(6 * var(--h-row-dense));
    max-height: calc(14 * var(--h-row-dense));
    overflow-y: auto;
    border: 1px solid var(--field-border);
    background: var(--surface-input);
  }

  .row {
    display: flex;
    align-items: center;
    min-width: 0;
    height: var(--h-row-dense);
    padding-right: var(--sp-2);
  }

  .name {
    min-width: 0;
  }

  .empty {
    padding: var(--sp-3);
  }

  .patterns {
    flex: 1 1 auto;
    min-height: calc(6 * var(--h-row-dense));
    resize: vertical;
    padding: var(--sp-2);
    border: 1px solid var(--field-border);
    background: var(--surface-input);
    color: var(--text-primary);
    font-family: var(--font-mono);
  }

  .preview {
    color: var(--text-primary);
  }

  .blocked {
    margin: var(--sp-2) 0 0;
    padding: 0;
    list-style: none;
  }

  .mono {
    font-family: var(--font-mono);
  }
</style>
