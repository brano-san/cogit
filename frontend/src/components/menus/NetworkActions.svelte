<script lang="ts">
  import PullDialog from "./PullDialog.svelte";
  import PushDialog from "./PushDialog.svelte";
  import { eachAtMost, FETCH_ALL_LANES } from "$lib/fetch-all";
  import { deleteMergedBranches, fetchRemote, listRemotes, reportTiming, type Branch, type RepoId } from "$lib/ipc";
  import { networkDefaults, saveNetworkDefaults } from "$lib/ipc/network-dialogs";
  import {
    EMPTY_DEFAULTS,
    fetchOptionsOf,
    mergeDefaults,
    pullChoiceOf,
    pullOptionsOf,
    pushChoiceOf,
    pushOptionsOf,
    pushTargetOf,
    type PullChoice,
    type PushChoice,
  } from "$lib/network-dialogs";
  import { pullFlow, pushFlow, type FlowUi } from "$lib/network-flow";
  import { noRemote, runGuarded, trackedRemote, type RunHost } from "$lib/network-runs";
  import type { BulkProgress } from "$lib/operations";
  import { fetchAllTargets } from "$lib/repo-list";
  import { measurer } from "$lib/timing";
  import { currentRemote, remotePlan, syncSteps, type SyncOrder } from "$lib/toolbar-prefs";
  import { confirmation } from "$stores/confirm.svelte";
  import { errors } from "$stores/errors.svelte";
  import { network } from "$stores/network.svelte";
  import { networkApi, networkDialog } from "$stores/network-dialog.svelte";
  import { repoPulse } from "$stores/repo-pulse.svelte";
  import { repository } from "$stores/repository.svelte";
  import { settings } from "$stores/settings.svelte";
  import { successToast } from "$stores/success-toast.svelte";
  import { toolbar } from "$stores/toolbar.svelte";

  /** Fetch, Pull, Push and Sync from the toolbar, the palette, Repositories and Output, with
      the Pull and Push dialogs. App lends its refreshes and the footer's hooks note. */
  interface Props {
    /** The local branch HEAD is on. */
    tracked: Branch | undefined;
    afterMutation: () => Promise<void>;
    afterFetch: (worked: RepoId) => Promise<void>;
    afterRefChange: (worked?: RepoId) => Promise<void>;
    pushNeedsDialog: () => boolean;
    openedModule: (key: string) => Promise<{ repo: RepoId } | null>;
    announceHooks: (id: RepoId, stage: "push") => Promise<void>;
    hooksDone: () => void;
  }

  let {
    tracked,
    afterMutation,
    afterFetch,
    afterRefChange,
    pushNeedsDialog,
    openedModule,
    announceHooks,
    hooksDone,
  }: Props = $props();

  const measure = measurer((label, ms, detail) => void reportTiming(label, ms, detail));

  let bulk = $state.raw<BulkProgress | undefined>(undefined);
  /** Fetch All's progress, for the footer. */
  export function bulkProgress(): BulkProgress | undefined {
    return bulk;
  }

  const pullRemote = $derived(currentRemote(tracked?.upstream, network.remotes));
  /** branch.<name>.pushRemote, else remote.pushDefault, as git itself pushes (D2). */
  const pushRemote = $derived(
    tracked?.pushRemote && network.remotes.includes(tracked.pushRemote) ? tracked.pushRemote : network.primary,
  );
  /** The remote Pull and Fetch use, for the toolbar's menus. */
  export function currentPullRemote(): string | null {
    return pullRemote;
  }

  const host: RunHost = {
    epoch: () => repository.epoch,
    report: (err, title) => errors.report(err, title),
    afterMutation: () => afterMutation(),
  };

  export async function run(kind: "fetch" | "pull" | "push") {
    // One Pull everywhere: the remote HEAD tracks and the fast-forward setting (#26).
    if (kind === "pull") return pullNow();
    if (kind === "push" && pushNeedsDialog()) return openPush();
    const id = repository.current?.repo;
    const root = repository.current?.root;
    const remote = kind === "fetch" ? pullRemote : pushRemote;
    if (!id || !root) return;
    if (!remote) {
      errors.message(...noRemote(kind));
      return;
    }
    let epoch: number | null;
    try {
      epoch = await runGuarded(host, `Could not ${kind}`, async () => {
        if (kind === "fetch") await network.fetch(id, remote);
        else {
          await announceHooks(id, "push");
          await pushWithDefaults(id, remote);
        }
      });
    } finally {
      hooksDone();
    }
    if (epoch === null) return;
    if (kind === "fetch") repoPulse.fetched(root);
    if (repository.epoch !== epoch) return;
    await (kind === "fetch" ? afterFetch(id) : afterRefChange(id));
  }

  /** Pull as the toolbar's own choices say: which remotes to fetch first, whether to delete
      merged branches afterwards, and the fast-forward setting from Preferences (#26). Each
      step waits for the one before; the first failure stops the rest. */
  async function runRemoteSteps(steps: readonly ("pull" | "push")[], failure: string) {
    const id = repository.current?.repo;
    const root = repository.current?.root;
    if (!id || !root) return;
    const ran = await runGuarded(host, failure, async () => {
      const plan = remotePlan(steps, {
        remotes: network.remotes,
        pullRemote,
        pushRemote,
        scope: toolbar.prefs.pullScope,
        ffOnly: settings.current.pullMode === "ffOnly",
        deleteMerged: toolbar.prefs.deleteMergedAfterPull,
        branch: tracked,
      });
      for (const step of plan) {
        if (step.kind === "fetch") await network.fetch(id, step.remote);
        if (step.kind === "pull") await pullWithDefaults(id, step.remote, step.ffOnly);
        if (step.kind === "deleteMerged") await deleteMergedBranches(id);
        if (step.kind === "push") await network.push(id, step.remote, false);
      }
    });
    if (ran === null) return;
    if (steps.includes("pull")) repoPulse.fetched(root);
    await afterRefChange(id);
  }

  const flowUi: FlowUi = {
    ask: (request) => confirmation.ask(request),
    report: (message, title) => errors.message(message, title),
  };

  /** What this repository remembers; a failed read means the plain defaults. */
  async function defaultsOf(id: RepoId) {
    return networkDefaults(id).catch((err) => {
      errors.report(err, "Could not read the remembered Pull and Push options");
      return EMPTY_DEFAULTS;
    });
  }

  async function pullWithDefaults(id: RepoId, remote: string, ffOnly: boolean) {
    const options = pullOptionsOf(pullChoiceOf(await defaultsOf(id)), ffOnly);
    await pullFlow(networkApi, flowUi, id, remote, options);
    successToast.show("Pull succeeded");
  }

  async function pushWithDefaults(id: RepoId, remote: string) {
    const head = tracked;
    if (!head) {
      await network.push(id, remote, false);
      return;
    }
    const target = pushTargetOf(head, network.remotes, network.primary);
    const choice = pushChoiceOf(await defaultsOf(id), {
      remote,
      local: head.name,
      branch: target.remote === remote ? target.branch : head.name,
      hasUpstream: head.upstream !== null,
    });
    await pushFlow(networkApi, flowUi, id, pushOptionsOf(choice));
    successToast.show("Push succeeded");
  }

  export async function openPull() {
    const id = repository.current?.repo;
    if (!id) return;
    if (!pullRemote) {
      errors.message(...noRemote("pull"));
      return;
    }
    networkDialog.open = {
      kind: "pull",
      repo: id,
      remote: pullRemote,
      remotes: network.remotes,
      ffOnly: settings.current.pullMode === "ffOnly",
      defaults: await defaultsOf(id),
    };
  }

  export async function openPush() {
    const id = repository.current?.repo;
    const head = tracked;
    if (!id) return;
    if (!head) {
      errors.message("HEAD is not on a branch. Check out the branch you want to push.", "Could not push");
      return;
    }
    const target = pushTargetOf(head, network.remotes, network.primary);
    if (!target.remote) {
      errors.message(...noRemote("push"));
      return;
    }
    networkDialog.open = {
      kind: "push",
      repo: id,
      remotes: network.remotes,
      remote: target.remote,
      branch: target.branch,
      local: head.name,
      upstream: head.upstream,
      remoteBranches: (repository.current?.branches ?? []).filter((b) => b.kind === "remote").map((b) => b.name),
      defaults: await defaultsOf(id),
    };
  }

  /** The dialog is gone first, so the footer shows the run. */
  async function runPullDialog(action: "pull" | "fetch", remote: string, choice: PullChoice, remember: boolean) {
    const request = networkDialog.open;
    const root = repository.current?.root;
    if (request?.kind !== "pull" || !root) return;
    networkDialog.close();
    const id = request.repo;
    const epoch = await runGuarded(host, action === "pull" ? "Could not pull" : "Could not fetch", async () => {
      if (remember) await saveNetworkDefaults(id, mergeDefaults(request.defaults, { pull: choice }));
      if (action === "fetch") {
        await pullFlow(networkApi, flowUi, id, remote, { fetchOnly: fetchOptionsOf(choice) });
        successToast.show("Fetch succeeded");
      } else {
        await pullFlow(networkApi, flowUi, id, remote, pullOptionsOf(choice, request.ffOnly));
        successToast.show("Pull succeeded");
      }
    });
    if (epoch === null) return;
    repoPulse.fetched(root);
    if (repository.epoch !== epoch) return;
    await (action === "fetch" ? afterFetch(id) : afterRefChange(id));
  }

  async function runPushDialog(choice: PushChoice, remember: boolean) {
    const request = networkDialog.open;
    if (request?.kind !== "push") return;
    networkDialog.close();
    const id = request.repo;
    let epoch: number | null;
    try {
      epoch = await runGuarded(host, "Could not push", async () => {
        if (remember) await saveNetworkDefaults(id, mergeDefaults(request.defaults, { push: choice }));
        await announceHooks(id, "push");
        await pushFlow(networkApi, flowUi, id, pushOptionsOf(choice));
        successToast.show("Push succeeded");
      });
    } finally {
      hooksDone();
    }
    if (epoch === null || repository.epoch !== epoch) return;
    await afterRefChange(id);
  }

  export function pullNow() {
    return runRemoteSteps(["pull"], "Could not pull");
  }

  /** Sync ▸ an order: runs it, and the Sync button runs it from then on (#27). */
  export function syncNow(order: SyncOrder = toolbar.prefs.syncOrder) {
    if (order !== toolbar.prefs.syncOrder) void toolbar.set("syncOrder", order);
    return runRemoteSteps(syncSteps(order), "Could not sync");
  }

  /** One failure does not stop the other remotes. */
  export async function fetchRemotes(names: readonly string[]) {
    const id = repository.current?.repo;
    const root = repository.current?.root;
    if (!id || !root) return;
    const upstream = pullRemote;
    for (const remote of names) {
      try {
        await network.fetch(id, remote);
        if (remote === upstream) repoPulse.fetched(root);
      } catch (err) {
        errors.report(err, `Could not fetch ${remote}`);
      }
    }
    await afterFetch(id);
  }

  /** One failure must not stop the rest: the point of Fetch All is not doing it by hand. */
  export async function fetchAll(marked: readonly string[]) {
    const targets = fetchAllTargets(marked, repository.openRepos);
    if (targets.length === 0) return;

    const watch = measure("fetch-all");
    let failed = 0;
    let done = 0;
    bulk = { label: "Fetching", done: 0, total: targets.length };

    await eachAtMost(targets, FETCH_ALL_LANES, async (entry) => {
      try {
        const remote = await trackedRemote(entry.repo);
        if (remote) {
          await fetchRemote(entry.repo, remote, () => {});
          repoPulse.fetched(entry.root);
        }
      } catch (err) {
        failed += 1;
        errors.report(err, "Could not fetch");
      }
      done += 1;
      bulk = { label: "Fetching", done, total: targets.length, failed };
    });

    bulk = undefined;
    watch.stop(`${targets.length} repositories, ${failed} failed`);
    await repository.refreshList();
    // The panels reload only when they show one of those fetched, and keep their selection.
    const shown = repository.current?.repo;
    if (shown && targets.some((entry) => entry.repo === shown)) await afterFetch(shown);
  }

  /** Pull or push the row's repository without bringing it to the front. */
  export async function syncListed(
    target:
      | { kind: "repository"; root: string; overview: { repo: RepoId } | null }
      | { kind: "submodule"; row: { key: string } },
    kind: "pull" | "push",
  ) {
    const overview = target.kind === "repository" ? target.overview : null;
    if (overview !== null && repository.current?.repo.valueOf() === overview.repo.valueOf()) {
      await run(kind);
      return;
    }
    const id =
      target.kind === "repository"
        ? (target.overview?.repo ?? null)
        : ((await openedModule(target.row.key))?.repo ?? null);
    if (id === null) return;
    // As the toolbar does for the one on screen: pull from the tracked remote, push to
    // origin (a fork's upstream is rarely writable).
    const remote =
      kind === "pull"
        ? await trackedRemote(id)
        : currentRemote(null, await listRemotes(id).catch(() => [] as string[]));
    if (!remote) {
      errors.message(...noRemote(kind));
      return;
    }
    try {
      if (kind === "pull") {
        await network.pull(id, remote, settings.current.pullMode === "ffOnly");
        if (target.kind === "repository") repoPulse.fetched(target.root);
      } else await network.push(id, remote, false);
    } catch (err) {
      errors.report(err, `Could not ${kind}`);
    }
    await repository.refreshList();
    if (target.kind === "repository") repoPulse.changed(target.root);
  }
</script>

{#if networkDialog.open?.kind === "pull"}
  <PullDialog request={networkDialog.open} onrun={runPullDialog} onclose={() => networkDialog.close()} />
{:else if networkDialog.open?.kind === "push"}
  <PushDialog request={networkDialog.open} onpush={runPushDialog} onclose={() => networkDialog.close()} />
{/if}
