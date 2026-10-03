<script lang="ts">
  import { open as openFolderDialog } from "@tauri-apps/plugin-dialog";
  import AddWorktreeDialog from "$components/repo-tree/AddWorktreeDialog.svelte";
  import RemoveWorktreeDialog from "$components/repo-tree/RemoveWorktreeDialog.svelte";
  import { popupContextMenu, type WorktreeBranch, type WorktreeEntry } from "$lib/ipc";
  import { revealOnDesktop } from "$lib/ipc/file-menus";
  import type { AddOrigin } from "$lib/worktree-add";
  import { parseWorktreeCommand, worktreeMenu } from "$lib/worktree-menu";
  import { removeWorktree, type RemovalHost } from "$lib/worktree-removal";
  import { commit } from "$stores/commit.svelte";
  import { confirmation } from "$stores/confirm.svelte";
  import { errors } from "$stores/errors.svelte";
  import { repository } from "$stores/repository.svelte";
  import { safety } from "$stores/safety.svelte";
  import { worktrees } from "$stores/worktrees.svelte";

  /** The Worktrees panel's menu, header buttons and palette entries, with the Add and Remove
      dialogs (R-184). App lends opening a row in the panels and the clipboard. */
  interface Props {
    openRow: (entry: WorktreeEntry) => Promise<void>;
    copyText: (text: string) => Promise<void>;
    /** Whether the graph has the focus: Add then starts at its selected commit. */
    graphFocused: boolean;
  }

  let { openRow, copyText, graphFocused }: Props = $props();

  let addOpen = $state(false);
  let addOrigin = $state<AddOrigin>({ kind: "current" });
  let removal = $state.raw<{ entry: WorktreeEntry } | null>(null);
  /** The row the menu was opened on, until its choice comes back as a menu command. */
  let target: WorktreeEntry | null = null;

  $effect(() =>
    repository.onLeave(() => {
      removal = null;
      addOpen = false;
    }),
  );

  const host: RemovalHost = {
    remove: (path, force) => worktrees.remove(path, force),
    fail: (error, title) => errors.report(error, title),
    refreshSafety: () => safety.refresh(),
    listed: (path) => worktrees.entries.some((entry) => entry.path === path),
    leftover: (path) => worktrees.leftover(path),
    deleteLeftover: (path) => worktrees.deleteLeftover(path),
    ask: (request) => confirmation.ask(request),
  };

  /** The dialog reads what is uncommitted itself and asks separately for --force (R-184). */
  export function remove(entry: WorktreeEntry) {
    removal = { entry };
  }

  async function confirmRemoval(force: boolean) {
    const entry = removal?.entry;
    removal = null;
    if (entry) await removeWorktree(entry, force, host);
  }

  export async function pruneAll() {
    const stale = worktrees.entries.filter((entry) => entry.missing);
    if (stale.length === 0) return;
    const names = stale.map((entry) => entry.name).join(", ");
    const confirmed = await confirmation.ask({
      title: "Prune Obsolete Worktrees",
      message:
        `Forget ${stale.length === 1 ? "the missing worktree" : `${stale.length} missing worktrees`} (${names})? ` +
        "Only Git's registration is removed; nothing on disk is touched. Locked ones are kept.",
      confirm: "Prune",
    });
    if (!confirmed) return;
    await worktrees.prune().catch((err) => errors.report(err, "Could not prune worktrees"));
  }

  export async function prune(entry: WorktreeEntry) {
    const confirmed = await confirmation.ask({
      title: "Prune Worktree",
      message:
        `Forget the worktree ${entry.name} at ${entry.path}? ` +
        "Only Git's registration of it is removed; nothing on disk is touched.",
      confirm: "Prune",
    });
    if (!confirmed) return;
    await worktrees.pruneOne(entry.path).catch((err) => errors.report(err, "Could not prune the worktree"));
  }

  /** Repair cannot guess where a folder went: the new place is asked for first. */
  export async function repair(entry: WorktreeEntry) {
    const picked = await openFolderDialog({
      directory: true,
      title: `Locate the folder of worktree ${entry.name}`,
    });
    if (typeof picked !== "string") return;
    await worktrees
      .repair(picked.replace(/\\/g, "/"))
      .catch((err) => errors.report(err, "Could not repair the worktree"));
  }

  /** A commit selected in the graph is the start; a menu on a branch or commit says its own. */
  export function openAdd(origin?: AddOrigin) {
    addOrigin = origin ?? (graphFocused && commit.oid ? { kind: "commit", oid: commit.oid } : { kind: "current" });
    addOpen = true;
  }

  async function addFrom(request: { path: string; branch: WorktreeBranch; open: boolean }) {
    addOpen = false;
    try {
      await worktrees.add(request.path, request.branch);
    } catch (err) {
      errors.report(err, "Could not add the worktree");
      return;
    }
    if (!request.open) return;
    const wanted = request.path.toLowerCase();
    const added = worktrees.entries.find((entry) => entry.path.toLowerCase() === wanted);
    if (added) await openRow(added);
  }

  export async function context(entry: WorktreeEntry, x: number, y: number) {
    target = entry;
    await popupContextMenu(worktreeMenu(entry), x, y).catch(() => {});
  }

  /** Returns true when the id belonged to a Worktrees row's menu and was handled here. */
  export function run(id: string): boolean {
    const entry = target;
    const command = parseWorktreeCommand(id);
    if (!entry || !command) return false;
    const failed = (what: string) => (err: unknown) => errors.report(err, `Could not ${what} the worktree`);
    switch (command) {
      case "open":
        void openRow(entry);
        break;
      case "copy":
        void copyText(entry.path);
        break;
      case "remove":
        remove(entry);
        break;
      case "prune":
        void prune(entry);
        break;
      case "repair":
        void repair(entry);
        break;
      case "lock":
        void worktrees.lock(entry.path, null).catch(failed("lock"));
        break;
      case "unlock":
        void worktrees.unlock(entry.path).catch(failed("unlock"));
        break;
      case "reveal":
        void revealOnDesktop(entry.path).catch(failed("reveal"));
        break;
    }
    return true;
  }
</script>

{#if addOpen && repository.current}
  {@const repo = repository.current}
  <AddWorktreeDialog
    repo={repo.repo}
    root={repo.root}
    branches={repo.branches}
    tags={repo.tags}
    worktrees={worktrees.entries}
    origin={addOrigin}
    onbrowse={async () => {
      const picked = await openFolderDialog({ directory: true, title: "Folder for the new worktree" });
      return typeof picked === "string" ? picked : null;
    }}
    onadd={(request) => void addFrom(request)}
    onclose={() => (addOpen = false)}
  />
{/if}

{#if removal}
  <RemoveWorktreeDialog
    entry={removal.entry}
    onremove={(force) => void confirmRemoval(force)}
    onclose={() => (removal = null)}
  />
{/if}
