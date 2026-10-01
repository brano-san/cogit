<script lang="ts">
  import PushBranchesDialog from "./PushBranchesDialog.svelte";
  import { cherryPick, commitDetails, popupContextMenu, revertCommits, type RepoId } from "$lib/ipc";
  import { pushTo } from "$lib/ipc/ref-ops";
  import { shortOid } from "$lib/format";
  import { appDeleteHost } from "$lib/ref-delete-host";
  import { runDeletion } from "$lib/ref-delete-run";
  import { branchPushes, type PushBranch } from "$lib/push-branches";
  import { buildRefTree, tickStates, type RefNode, type RefTreeInput } from "$lib/ref-nodes";
  import {
    SELECTION_MENU_PREFIX,
    branchSelectionMenu,
    commitSelectionMenu,
    refPicks,
    replayOrder,
    selectionDeletable,
    selectionDeletion,
    selectionQuestion,
    toggleSelected,
  } from "$lib/ref-selection";
  import { fullMessage } from "$lib/rewrite-plans";
  import { confirmation } from "$stores/confirm.svelte";
  import { errors } from "$stores/errors.svelte";
  import { graph } from "$stores/graph.svelte";
  import { network } from "$stores/network.svelte";
  import { notices } from "$stores/notices.svelte";
  import { refs } from "$stores/refs.svelte";
  import { repository } from "$stores/repository.svelte";

  /** The menus of several selected commits (Graph) or rows (Branches), and what they run.
      The single-row menus stay with `RefActions`; the choice is made by the lists. */
  interface Props {
    /** What the tree in Branches is built from: Toggle counts the same rows its boxes do. */
    input: RefTreeInput;
    afterRefChange: (worked?: RepoId) => Promise<void>;
    reloadGraph: () => Promise<void>;
  }

  let { input, afterRefChange, reloadGraph }: Props = $props();

  type Target = { kind: "commits"; oids: readonly string[] } | { kind: "refs"; nodes: readonly RefNode[] };

  /** The chosen id comes back as a `menu-command` event, after the popup has closed. */
  let target: Target | null = null;
  let pushing = $state.raw<readonly PushBranch[] | null>(null);
  const tree = $derived(buildRefTree(input));

  $effect(() =>
    repository.onLeave(() => {
      target = null;
      pushing = null;
    }),
  );

  /** `oids` in any order: what the menu needs is only that there are several. */
  export async function commitsContext(oids: readonly string[], x: number, y: number) {
    target = { kind: "commits", oids };
    await popupContextMenu(commitSelectionMenu(), x, y).catch(() => {});
  }

  export async function refsContext(nodes: readonly RefNode[], x: number, y: number) {
    target = { kind: "refs", nodes };
    const picks = refPicks(nodes);
    const ticks = tickStates(tree, refs.visible);
    const items = branchSelectionMenu({
      picks,
      hasRemote: network.remotes.length > 0,
      deletable: selectionDeletable(selectionDeletion(nodes, network.remotes)),
      toggle: nodes.some((node) => ticks.get(node.id)?.tickable) ? null : "nothing here is drawn in the graph",
    });
    await popupContextMenu(items, x, y).catch(() => {});
  }

  /** True when the id came from one of these menus and has been taken care of. */
  export function run(id: string): boolean {
    if (!id.startsWith(SELECTION_MENU_PREFIX)) return false;
    void act(id.slice(SELECTION_MENU_PREFIX.length));
    return true;
  }

  export function toggleRows(nodes: readonly RefNode[]) {
    refs.set(toggleSelected(tree, nodes.map((node) => node.id), refs.visible));
    return reloadGraph();
  }

  async function attempt(title: string, step: () => Promise<unknown>): Promise<boolean> {
    try {
      await step();
      return true;
    } catch (err) {
      errors.report(err, title);
      return false;
    }
  }

  async function copy(text: string) {
    if (text === "") return;
    const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
    await writeText(text).catch((err: unknown) => errors.report(err, "Could not copy"));
  }

  /** The graph row of each commit: a larger row is an older commit. */
  async function rowsOf(oids: readonly string[]): Promise<Map<string, number>> {
    const found = await Promise.all(
      oids.map(async (oid) => [oid, graph.loadedIndexOf(oid) ?? (await graph.indexOf(oid))] as const),
    );
    return new Map(found.map(([oid, row]) => [oid, row ?? 0]));
  }

  function summaryOf(oid: string): string {
    const at = graph.loadedIndexOf(oid);
    const summary = at === null ? undefined : graph.rowAt(at)?.commit.summary;
    return summary ? `${shortOid(oid)} ${summary}` : shortOid(oid);
  }

  async function replay(id: RepoId, oids: readonly string[], pick: boolean) {
    const rows = await rowsOf(oids);
    const ordered = replayOrder(rows, oids, pick ? "oldest-first" : "newest-first");
    const verb = pick ? "Cherry-Pick" : "Revert";
    const go = await confirmation.ask({
      title: `${verb} Commits`,
      message: pick
        ? `Cherry-pick ${ordered.length} commits onto the current branch, oldest first?`
        : `Revert ${ordered.length} commits, newest first? Each makes a commit of its own.`,
      confirm: verb,
      items: ordered.map(summaryOf),
    });
    if (!go) return;
    await attempt(pick ? "Cherry-pick failed" : "Revert failed", () =>
      pick ? cherryPick(id, ordered) : revertCommits(id, ordered),
    );
    await afterRefChange(id);
  }

  async function act(name: string) {
    const id = repository.current?.repo;
    const at = target;
    if (!id || !at) return;
    if (at.kind === "commits") {
      switch (name) {
        case "cherry-pick":
          return replay(id, at.oids, true);
        case "revert":
          return replay(id, at.oids, false);
        case "copy-id":
          return copy(replayOrder(await rowsOf(at.oids), at.oids, "newest-first").join("\n"));
        case "copy-message": {
          const ordered = replayOrder(await rowsOf(at.oids), at.oids, "newest-first");
          try {
            const details = await Promise.all(ordered.map((oid) => commitDetails(id, oid)));
            return copy(details.map(fullMessage).join("\n\n"));
          } catch (err) {
            errors.report(err, "Could not copy the messages");
            return;
          }
        }
      }
      return;
    }
    switch (name) {
      case "push-to": {
        const locals = refPicks(at.nodes).locals.flatMap((node) =>
          node.branch ? [{ name: node.branch.name, upstream: node.branch.upstream ?? null }] : [],
        );
        if (locals.length > 0) pushing = locals;
        return;
      }
      case "delete":
        return deleteRefs(id, at.nodes);
      case "copy": {
        const picks = refPicks(at.nodes);
        const names = at.nodes.flatMap((node) =>
          [...picks.locals, ...picks.remotes].includes(node)
            ? [node.branch?.name ?? ""]
            : picks.tags.includes(node)
              ? [node.tag?.name ?? ""]
              : [],
        );
        return copy(names.join("\n"));
      }
      case "toggle":
        return toggleRows(at.nodes);
    }
  }

  async function deleteRefs(id: RepoId, nodes: readonly RefNode[]) {
    const plans = selectionDeletion(nodes, network.remotes).filter((plan) => plan.names.length > 0);
    if (plans.length === 0) return;
    const go = await confirmation.ask({
      title: "Delete Refs",
      message: selectionQuestion(selectionDeletion(nodes, network.remotes)),
      confirm: "Delete",
      warning: true,
      items: plans.flatMap((plan) => plan.names),
    });
    if (!go) return;
    let deleted = 0;
    const failed: string[] = [];
    for (const plan of plans) {
      const result = await runDeletion(
        { kind: plan.kind, remote: plan.remote, names: plan.names, force: false },
        appDeleteHost(id),
      );
      deleted += result.deleted;
      failed.push(...result.failed);
    }
    if (failed.length > 0) {
      notices.inform("Some refs were not deleted", `${deleted} deleted, ${failed.length} failed: ${failed.join(", ")}.`);
    }
    await afterRefChange(id);
  }

  async function sendPush(remote: string, track: boolean) {
    const id = repository.current?.repo;
    const branches = pushing;
    pushing = null;
    if (!id || !branches) return;
    const pushes = branchPushes(branches, remote, network.remotes, track);
    await attempt("Could not push", () =>
      network.run(id, "Pushing", async (onLine) => {
        for (const each of pushes) await pushTo(id, remote, each.refspec, each.track, onLine);
      }),
    );
    await afterRefChange(id);
  }
</script>

{#if pushing}
  <PushBranchesDialog
    branches={pushing}
    remotes={network.remotes}
    primary={network.primary}
    onpush={(remote, track) => void sendPush(remote, track)}
    onclose={() => (pushing = null)}
  />
{/if}
