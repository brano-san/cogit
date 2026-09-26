<script lang="ts">
  import { untrack } from "svelte";
  import BisectFinishedDialog from "./BisectFinishedDialog.svelte";
  import StartBisectDialog from "./StartBisectDialog.svelte";
  import {
    bisectMenuChoice,
    bisectOf,
    newlyFound,
    resetQuestion,
    startBlocked,
    startFields,
    type BisectAction,
  } from "$lib/bisect";
  import { commitDetails, type CommitDetails, type RepoId } from "$lib/ipc";
  import { bisectMark, bisectReset, bisectStart, type BisectMark } from "$lib/ipc/bisect";
  import type { BannerAction } from "$lib/repo-state";
  import { commit } from "$stores/commit.svelte";
  import { confirmation } from "$stores/confirm.svelte";
  import { errors } from "$stores/errors.svelte";
  import { graph } from "$stores/graph.svelte";
  import { repository } from "$stores/repository.svelte";
  import { stashView } from "$stores/stash-view.svelte";

  /** `git bisect` from the graph's menu, Branch ▸ Bisect, the palette and the banner, with
      its two dialogs (F-565, F-567, F-568). App lends only its refresh after a ref change. */
  interface Props {
    afterRefChange: (worked?: RepoId) => Promise<void>;
  }

  let { afterRefChange }: Props = $props();

  let starting = $state.raw<{ bad: string; good: string } | null>(null);
  let finished = $state.raw<{ commit: CommitDetails; term: string } | null>(null);
  let busy = $state(false);

  const BANNER: Partial<Record<BannerAction, BisectAction | "show">> = {
    markGood: "good",
    markBad: "bad",
    markSkip: "skip",
    resetBisect: "reset",
    showFirstBad: "show",
  };

  /** True when the id came from the Bisect submenu of a commit and has been taken care of. */
  export function run(id: string): boolean {
    const choice = bisectMenuChoice(id);
    if (!choice) return false;
    void act(choice.action, choice.oid);
    return true;
  }

  /** Branch ▸ Bisect and the palette: the marks act on HEAD. */
  export function command(action: BisectAction) {
    void act(action, null);
  }

  export function claims(action: BannerAction): boolean {
    return BANNER[action] !== undefined;
  }

  export function banner(action: BannerAction) {
    const chosen = BANNER[action];
    if (chosen === "show") revealFound();
    else if (chosen) void act(chosen, null);
  }

  async function act(action: BisectAction, oid: string | null) {
    const id = repository.current?.repo;
    if (!id) return;
    if (action === "start") openStart(oid);
    else if (action === "reset") await reset(id);
    else await mark(id, action, oid);
  }

  function openStart(oid: string | null) {
    const summary = repository.current;
    if (!summary || startBlocked(summary.state) !== null) return;
    const head = summary.head.kind === "unborn" ? null : summary.head.oid;
    starting = startFields(oid ?? commit.oid, head);
  }

  async function resolve(rev: string): Promise<{ oid: string; summary: string } | null> {
    const id = repository.current?.repo;
    if (!id) return null;
    try {
      const details = await commitDetails(id, rev);
      return { oid: details.oid, summary: details.summary };
    } catch {
      return null;
    }
  }

  async function begin(bad: string, good: string) {
    const id = repository.current?.repo;
    if (!id || busy) return;
    busy = true;
    try {
      await bisectStart(id, bad.trim(), good.trim() === "" ? null : good.trim());
    } catch (err) {
      errors.report(err, "Could not start the bisect");
    } finally {
      busy = false;
      starting = null;
    }
    await afterRefChange(id);
  }

  async function mark(id: RepoId, mark: BisectMark, oid: string | null) {
    const before = bisectOf(repository.current?.state)?.firstBad ?? null;
    try {
      await bisectMark(id, mark, oid);
    } catch (err) {
      errors.report(err, mark === "skip" ? "Could not skip the commit" : `Could not mark the commit ${mark}`);
    }
    await afterRefChange(id);
    // Only a step of ours ends in the dialog: one taken in a terminal is said by the banner.
    const now = bisectOf(repository.current?.state);
    if (repository.current?.repo === id && now?.firstBad && now.firstBad !== before) {
      await showFinished(id, now.firstBad, now.terms.bad);
    }
  }

  async function showFinished(id: RepoId, oid: string, term: string) {
    try {
      finished = { commit: await commitDetails(id, oid), term };
    } catch (err) {
      errors.report(err, "Could not read the first bad commit");
    }
  }

  async function reset(id: RepoId) {
    const bisect = bisectOf(repository.current?.state);
    if (!bisect) return;
    const question = resetQuestion(bisect);
    if (question && !(await confirmation.ask(question))) return;
    if (repository.current?.repo !== id) return;
    busy = true;
    try {
      await bisectReset(id);
    } catch (err) {
      errors.report(err, "Could not reset the bisect");
    } finally {
      busy = false;
      finished = null;
    }
    await afterRefChange(id);
  }

  function reveal(id: RepoId, oid: string) {
    stashView.clear();
    void commit.select(id, oid);
    graph.requestReveal(oid);
  }

  function revealFound() {
    const id = repository.current?.repo;
    const found = bisectOf(repository.current?.state)?.firstBad;
    if (id && found) reveal(id, found);
  }

  async function copyId(oid: string) {
    const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
    await writeText(oid).catch((err: unknown) => errors.report(err, "Could not copy"));
  }

  /** The end of the search selects its answer, whoever took the last step (F-568). */
  let seen: { repo: string; firstBad: string | null } | null = null;
  $effect(() => {
    const current = repository.current;
    const root = current?.root ?? "";
    const bisect = bisectOf(current?.state);
    const found = newlyFound(seen, root, bisect);
    seen = { repo: root, firstBad: bisect?.firstBad ?? null };
    if (found && current) untrack(() => reveal(current.repo, found));
  });
</script>

{#if starting}
  <StartBisectDialog
    bad={starting.bad}
    good={starting.good}
    {busy}
    {resolve}
    onstart={(bad, good) => void begin(bad, good)}
    onclose={() => (starting = null)}
  />
{/if}

{#if finished}
  {@const found = finished.commit}
  <BisectFinishedDialog
    {found}
    term={finished.term}
    {busy}
    oncopy={() => void copyId(found.oid)}
    onleave={() => {
      const id = repository.current?.repo;
      if (id) void reset(id);
    }}
    onclose={() => (finished = null)}
  />
{/if}
