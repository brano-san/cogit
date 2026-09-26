<script lang="ts">
  import ConfirmDialog from "$components/common/ConfirmDialog.svelte";
  import AddTagDialog from "./AddTagDialog.svelte";
  import EditAuthorDialog from "./EditAuthorDialog.svelte";
  import EditMessageDialog from "./EditMessageDialog.svelte";
  import PushToDialog from "./PushToDialog.svelte";
  import ResetDialog from "./ResetDialog.svelte";
  import {
    CogitError,
    checkout,
    cherryPick,
    commitDetails,
    createBranch,
    createTag,
    deleteBranch,
    deleteRemoteBranch,
    deleteTag,
    interactiveRebase,
    isPublished,
    mergeInto,
    popupContextMenu,
    protectingRefs,
    rebaseOnto,
    rebaseTodo,
    renameBranch,
    revertCommits,
    type Branch,
    type CommitDetails,
    type ContextItem,
    type RepoId,
    type Tag,
  } from "$lib/ipc";
  import {
    editAuthor,
    isAncestor,
    pushTo,
    renameStash,
    renameTag,
    resetTo,
    tagMessage,
    tagNameProblem,
    type ResetMode,
  } from "$lib/ipc/ref-ops";
  import { shortOid, type RefLabel } from "$lib/format";
  import type { RefNode } from "$lib/ref-nodes";
  import { initialRemote, pushRefspec, pushUpTo, splitUpstream, type PushSource } from "$lib/push-to";
  import {
    REF_MENU_PREFIX,
    branchesBranchMenu,
    branchesStashMenu,
    branchesTagMenu,
    commitFacts,
    graphCommitMenu,
    graphRefMenu,
    labelTarget,
    workingTreeMenu,
    type CommitFacts,
    type RefTarget,
  } from "$lib/ref-menus";
  import { checkoutPlan, nodeTarget, type NodeTarget } from "$lib/ref-checkout";
  import { publishedOrAssume } from "$lib/published";
  import { resetChoice } from "$lib/reset-modes";
  import { baseBefore, fullMessage, modifyPlan, rewordPlan, squashPlan } from "$lib/rewrite-plans";
  import { tagRequest } from "$lib/tag-dialog";
  import { branchRevision } from "$lib/toolbar";
  import { branchNameProblem, textProblem } from "$lib/names";
  import { commit } from "$stores/commit.svelte";
  import { compareView } from "$stores/compare-view.svelte";
  import { confirmation } from "$stores/confirm.svelte";
  import { errors } from "$stores/errors.svelte";
  import { graph } from "$stores/graph.svelte";
  import { network } from "$stores/network.svelte";
  import { notices } from "$stores/notices.svelte";
  import { prompt } from "$stores/prompt.svelte";
  import { refDialogs } from "$stores/ref-dialogs.svelte";
  import { commitBox } from "$stores/commit-box.svelte";
  import { runWorkingTreeAction } from "$lib/working-tree-actions";
  import { refs } from "$stores/refs.svelte";
  import { repository } from "$stores/repository.svelte";
  import { stashView } from "$stores/stash-view.svelte";
  import { stashes } from "$stores/stashes.svelte";
  import { worktree } from "$stores/worktree.svelte";

  /** The graph and Branches context menus (#33–#35, #37–#39) and their dialogs (#6,
      #28, Reset Advanced…). App keeps only the wiring: it hands over the right-click and
      the chosen item id, and lends the flows it already owns. */
  interface Props {
    afterRefChange: () => Promise<void>;
    afterMutation: () => Promise<void>;
    /** App's write to the working tree: it names the paths, so the open diff is read again. */
    mutate: (step: (repo: RepoId) => Promise<unknown>, paths: string[], readsBack: boolean) => Promise<boolean>;
    reloadGraph: () => Promise<void>;
    /** App's switch: it knows about worktrees holding the branch and about autostash. */
    checkoutBranch: (branch: Branch) => Promise<void>;
    /** Open App's dialogs for the selected commit. */
    openSplit: () => Promise<void>;
    openRebase: () => Promise<void>;
    rollbackTree: () => Promise<void>;
  }

  let {
    afterRefChange,
    afterMutation,
    mutate,
    reloadGraph,
    checkoutBranch,
    openSplit,
    openRebase,
    rollbackTree,
  }: Props = $props();

  interface Target {
    oid: string | null;
    details: CommitDetails | null;
    facts: CommitFacts;
    ref: RefTarget | null;
    branch: Branch | null;
    tag: Tag | null;
    stash: { index: number; message: string } | null;
    node: RefNode | null;
    /** What the graph had selected when the menu opened. */
    selected: string | null;
  }

  const NO_COMMIT: CommitFacts = {
    isHeadCommit: false,
    detachedHere: false,
    onHead: false,
    published: false,
    parents: 0,
    upstream: null,
    hasRemote: false,
  };

  /** The chosen id comes back as a `menu-command` event, after the popup has closed. */
  let target: Target | null = null;
  /** A second right-click before the first menu's facts arrive wins. */
  let asked = 0;



  function repoId(): RepoId | null {
    return repository.current?.repo ?? null;
  }

  async function factsOf(
    id: RepoId,
    oid: string,
    withPublished: boolean,
  ): Promise<{ details: CommitDetails; facts: CommitFacts }> {
    const [details, onHead, published, protectedBy] = await Promise.all([
      commitDetails(id, oid),
      isAncestor(id, oid, "HEAD").catch(() => false),
      withPublished ? publishedOrAssume(isPublished(id, oid)) : Promise.resolve(false),
      withPublished ? protectingRefs(id, oid).catch(() => [] as string[]) : Promise.resolve([] as string[]),
    ]);
    const summary = repository.current;
    const facts = commitFacts({
      oid,
      parents: details.parents.length,
      head: summary?.head,
      branches: summary?.branches ?? [],
      onHead,
      published,
      hasRemote: network.remotes.length > 0,
      protectedBy,
    });
    return { details, facts };
  }

  async function show(next: Target, items: ContextItem[], x: number, y: number) {
    target = next;
    await popupContextMenu(items, x, y).catch(() => {});
  }

  function blank(): Omit<Target, "facts"> {
    return {
      oid: null,
      details: null,
      ref: null,
      branch: null,
      tag: null,
      stash: null,
      node: null,
      selected: commit.oid,
    };
  }

  /** What Merge and Rebase hand git: a branch by a name git cannot take for a tag. */
  function revisionOf(at: Target, oid: string): string {
    const summary = repository.current;
    if (at.branch && summary) return branchRevision(at.branch, summary.branches, summary.tags);
    return at.ref?.name ?? oid;
  }

  export async function worktreeContext(x: number, y: number) {
    const status = repository.current?.status;
    if (!status) return;
    asked += 1;
    await show({ ...blank(), facts: NO_COMMIT }, workingTreeMenu({
      staged: status.staged,
      modified: status.unstaged,
      untracked: status.untracked,
    }), x, y);
  }

  export async function commitContext(oid: string, x: number, y: number) {
    const id = repoId();
    if (!id) return;
    const token = ++asked;
    try {
      const { details, facts } = await factsOf(id, oid, true);
      if (token !== asked) return;
      await show({ ...blank(), oid, details, facts }, graphCommitMenu(facts), x, y);
    } catch (err) {
      errors.report(err, "Could not open the commit menu");
    }
  }

  export async function labelContext(label: RefLabel, oid: string, x: number, y: number) {
    const id = repoId();
    const summary = repository.current;
    if (!id || !summary) return;
    if (label.kind === "stash") {
      await stashLabelContext(label.text, oid, x, y);
      return;
    }
    const found = labelTarget(label, summary.branches, summary.tags);
    if (!found) return;
    const token = ++asked;
    try {
      const { details, facts } = await factsOf(id, oid, true);
      if (token !== asked) return;
      await show({ ...blank(), ...found, oid, details, facts }, graphRefMenu(found.ref, facts), x, y);
    } catch (err) {
      errors.report(err, "Could not open the ref menu");
    }
  }

  /** A stash label in the graph gets the stash menu of Branches (#35). */
  async function stashLabelContext(text: string, oid: string, x: number, y: number) {
    const index = Number(/\{(\d+)\}/.exec(text)?.[1] ?? Number.NaN);
    if (Number.isNaN(index)) return;
    const node: RefNode = { id: `stash:${index}`, kind: "stash", label: text, depth: 1, rev: text, oid };
    await branchesContext(node, x, y);
  }

  /** The Branches rows this component has a menu for. */
  export function claims(node: RefNode): boolean {
    return node.kind === "local" || node.kind === "remote" || node.kind === "tag" || node.kind === "stash";
  }

  export async function branchesContext(node: RefNode, x: number, y: number) {
    const id = repoId();
    const summary = repository.current;
    if (!id || !summary || !claims(node)) return;
    const token = ++asked;
    const found = nodeTarget(node, summary.tags);
    const tag = found?.tag ?? undefined;
    const oid = found ? found.oid : (node.oid ?? null);
    try {
      const loaded = oid ? await factsOf(id, oid, false) : null;
      if (token !== asked) return;
      const facts = loaded?.facts ?? { ...NO_COMMIT, hasRemote: network.remotes.length > 0 };
      const at = { selected: commit.oid, oid, untickable: node.disabled ?? null };
      const base = { ...blank(), oid, details: loaded?.details ?? null, facts, node };

      if (node.kind === "stash") {
        const index = Number(node.id.slice("stash:".length));
        const message = stashes.entries.find((entry) => entry.index === index)?.message ?? "";
        await show({ ...base, stash: { index, message } }, branchesStashMenu(facts, at), x, y);
      } else if (node.kind === "tag") {
        if (!tag || !found?.ref) return;
        await show({ ...base, ref: found.ref, tag }, branchesTagMenu(facts, { ...at, annotated: tag.isAnnotated }), x, y);
      } else if (found?.ref && found.branch) {
        const { ref, branch } = found;
        await show({ ...base, ref, branch }, branchesBranchMenu(ref, facts, at), x, y);
      }
    } catch (err) {
      errors.report(err, "Could not open the menu");
    }
  }

  /** True when the id came from one of these menus and has been taken care of. */
  export function run(id: string): boolean {
    if (!id.startsWith(REF_MENU_PREFIX)) return false;
    void act(id.slice(REF_MENU_PREFIX.length));
    return true;
  }

  /** #6: Add Tag for a commit, or for the selected commit, or for HEAD. */
  export async function addTag(oid: string | null) {
    const id = repoId();
    const head = repository.current?.head;
    const at = oid ?? commit.oid ?? (head && head.kind !== "unborn" ? head.oid : null);
    if (!id || !at) {
      errors.message("There is no commit to tag yet.", "Could not add a tag");
      return;
    }
    try {
      const details = await commitDetails(id, at);
      refDialogs.tag = { oid: details.oid, subject: details.summary };
    } catch (err) {
      errors.report(err, "Could not add a tag");
    }
  }

  /** #28: Push To… for the checked-out branch, from the toolbar. */
  export function pushToCurrent() {
    const summary = repository.current;
    const head = summary?.head;
    if (!summary || head?.kind !== "branch") {
      errors.message("HEAD is not on a branch. Check out the branch you want to push.", "Push To");
      return;
    }
    const branch = summary.branches.find((entry) => entry.kind === "local" && entry.name === head.name);
    refDialogs.push = { kind: "branch", name: head.name, upstream: branch?.upstream ?? null };
  }

  async function attempt(
    title: string,
    step: () => Promise<unknown>,
    after: () => Promise<void> = afterRefChange,
  ): Promise<boolean> {
    let done = true;
    try {
      await step();
    } catch (err) {
      errors.report(err, title);
      done = false;
    }
    await after();
    return done;
  }

  async function copy(text: string) {
    if (text === "") return;
    const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
    await writeText(text).catch((err: unknown) => errors.report(err, "Could not copy"));
  }

  function notOnBranch(): CogitError {
    return new CogitError({ kind: "invalidState", data: "the commit is not on the checked-out branch" });
  }

  /** "With a warning if pushed": rewriting shared history is asked about first. */
  async function publishedOk(at: Target, action: string): Promise<boolean> {
    if (!at.facts.published || !at.oid) return true;
    return confirmation.ask({
      title: `${action} a Pushed Commit`,
      message:
        `${shortOid(at.oid)} is already on a remote branch. ${action} rewrites it and every commit ` +
        "after it, so the branch will need a force-push and everyone who pulled it will have to reset.",
      confirm: action,
      warning: true,
    });
  }

  async function act(name: string) {
    const id = repoId();
    if (!id) return;
    if (name.startsWith("wt-")) {
      await worktreeAction(id, name);
      return;
    }
    const at = target;
    if (!at) return;
    const oid = at.oid;

    switch (name) {
      case "checkout":
        return checkoutTarget(id, at);
      case "merge":
        if (oid) await attempt("Could not merge", () =>
          mergeInto(id, { source: revisionOf(at, oid), noFastForward: false, squash: false, message: null }),
        );
        return;
      case "cherry-pick":
        if (oid) await attempt("Cherry-pick failed", () => cherryPick(id, [oid]));
        return;
      case "revert":
        if (oid) await attempt("Revert failed", () => revertCommits(id, [oid]));
        return;
      case "rebase":
        if (oid) await attempt("Could not rebase", () => rebaseOnto(id, { onto: revisionOf(at, oid), autostash: true }));
        return;
      case "modify":
        return modify(id, at);
      case "split":
        if (oid && (await publishedOk(at, "Split"))) {
          stashView.clear();
          await commit.select(id, oid);
          await openSplit();
        }
        return;
      case "squash":
        return squash(id, at);
      case "edit-message":
        if (oid && at.details && (await publishedOk(at, "Edit Message"))) {
          refDialogs.message = { oid, message: fullMessage(at.details), parents: at.details.parents };
        }
        return;
      case "edit-author":
        if (oid && at.details && (await publishedOk(at, "Edit Author"))) {
          refDialogs.author = { oid, name: at.details.author.name, email: at.details.author.email };
        }
        return;
      case "rebase-i":
        if (oid) {
          stashView.clear();
          await commit.select(id, oid);
          await openRebase();
        }
        return;
      case "rollback":
        if (oid) {
          stashView.clear();
          await commit.select(id, oid);
          await rollbackTree();
        }
        return;
      case "add-branch":
        return addBranch(id, at);
      case "add-tag":
        return addTag(oid);
      case "reset":
        if (oid) await attempt("Could not reset", () => resetTo(id, oid, "mixed"));
        return;
      case "reset-advanced":
        if (oid) refDialogs.reset = { oid, subject: at.details?.summary ?? "", moving: movingRef() };
        return;
      case "push-up-to":
        return pushUpToTarget(id, at);
      case "push":
        return pushTarget(id, at);
      case "push-to": {
        const source = pushSourceOf(at);
        if (source) refDialogs.push = source;
        return;
      }
      case "delete":
        return deleteTarget(id, at);
      case "rename":
        return renameTarget(id, at);
      case "copy-name":
        return copy(at.ref?.name ?? "");
      case "copy-message":
        return copy(at.details ? fullMessage(at.details) : "");
      case "copy-id":
        return copy(oid ?? "");
      case "reveal":
        if (oid && at.node) await reveal(id, at.node, oid);
        return;
      case "compare-head": {
        const head = repository.current?.head;
        if (oid && head && head.kind !== "unborn") await compare(id, head.oid, oid, null);
        return;
      }
      case "compare-selected":
        if (oid && at.selected) await compare(id, at.selected, oid, at.node);
        return;
      case "toggle":
        if (at.node) await toggle(at.node);
        return;
      case "copy-tag-message":
        if (at.tag) {
          const text = await tagMessage(id, at.tag.name).catch((err) => {
            errors.report(err, "Could not read the tag message");
            return null;
          });
          if (text) await copy(text);
        }
        return;
      case "apply-stash":
      case "pop-stash":
        if (at.stash) {
          const index = at.stash.index;
          const pop = name === "pop-stash";
          await attempt(pop ? "Could not pop the stash" : "Could not apply the stash", () =>
            stashes.apply(id, index, pop),
          );
        }
        return;
      case "rename-stash":
        return renameStashTarget(id, at);
      case "drop-stash":
        return dropStashTarget(id, at);
      case "copy-stash-message":
        return copy(at.stash?.message ?? "");
    }
  }

  function movingRef(): string {
    const head = repository.current?.head;
    return head?.kind === "branch" ? head.name : "HEAD";
  }

  async function checkoutTarget(id: RepoId, at: NodeTarget) {
    const plan = checkoutPlan(at, network.remotes);
    if (plan === null) return;
    if (plan.kind === "switch") return checkoutBranch(plan.branch);
    const go = await confirmation.ask({
      title: "Check Out",
      message:
        `Check out ${plan.what}? HEAD will be detached: commits made from there belong to no branch ` +
        "until you add one, and are easy to lose when you switch away.",
      confirm: "Check Out",
    });
    if (go) await attempt("Could not check out", () => checkout(id, { kind: "commit", oid: plan.oid }));
  }

  /** A double click in Branches is the menu's Check Out: a remote branch as its local one,
      a tag only after the question about detaching HEAD. */
  export async function checkOutNode(node: RefNode) {
    const id = repoId();
    const found = nodeTarget(node, repository.current?.tags ?? []);
    if (id && found) await checkoutTarget(id, found);
  }

  async function modify(id: RepoId, at: Target) {
    const oid = at.oid;
    if (!oid || !at.details || !(await publishedOk(at, "Modify"))) return;
    const base = baseBefore(oid, at.details.parents);
    await attempt("Could not start Modify", async () => {
      const plan = modifyPlan(await rebaseTodo(id, base), oid);
      if (!plan) throw notOnBranch();
      await interactiveRebase(id, base, plan, false);
    });
  }

  /** Into the parent: the plan starts one commit earlier so that it holds both. */
  async function squash(id: RepoId, at: Target) {
    const oid = at.oid;
    const parent = at.details?.parents[0];
    if (!oid || !parent) return;
    const go = await confirmation.ask({
      title: "Squash",
      message: `Squash ${shortOid(oid)} into its parent ${shortOid(parent)}? The two become one commit carrying both messages.`,
      confirm: "Squash",
    });
    if (!go) return;
    await attempt("Could not squash", async () => {
      const base = baseBefore(parent, (await commitDetails(id, parent)).parents);
      const plan = squashPlan(await rebaseTodo(id, base), oid);
      if (!plan) throw notOnBranch();
      await interactiveRebase(id, base, plan, false);
    });
  }

  async function saveMessage(message: string) {
    const id = repoId();
    const dialog = refDialogs.message;
    refDialogs.message = null;
    if (!id || !dialog) return;
    const base = baseBefore(dialog.oid, dialog.parents);
    await attempt("Could not edit the message", async () => {
      const plan = rewordPlan(await rebaseTodo(id, base), dialog.oid, message);
      if (!plan) throw notOnBranch();
      await interactiveRebase(id, base, plan, false);
    });
  }

  async function saveAuthor(name: string, email: string) {
    const id = repoId();
    const dialog = refDialogs.author;
    refDialogs.author = null;
    if (!id || !dialog) return;
    await attempt("Could not edit the author", () => editAuthor(id, dialog.oid, name, email));
  }

  async function addBranch(id: RepoId, at: Target) {
    const oid = at.oid;
    if (!oid) return;
    const name = await prompt.ask({
      title: `Add Branch at ${shortOid(oid)}`,
      label: "Name",
      confirm: "Add Branch",
      validate: (value) => branchNameProblem(value, repository.localBranches.map((entry) => entry.name)),
    });
    if (name === null) return;
    await attempt("Could not add the branch", () => createBranch(id, name, oid, false));
  }

  async function createTagFrom(name: string, message: string) {
    const id = repoId();
    const dialog = refDialogs.tag;
    if (!id || !dialog) return;
    try {
      await createTag(id, tagRequest(name, message, dialog.oid));
    } catch (err) {
      errors.report(err, "Could not add the tag");
      return;
    }
    refDialogs.tag = null;
    await afterRefChange();
  }

  async function checkTagName(name: string): Promise<string | null> {
    const id = repoId();
    if (!id) return "No repository is open.";
    return tagNameProblem(id, name).catch((err) => {
      errors.report(err, "Could not check the tag name");
      return "Git could not check the name.";
    });
  }

  async function resetWith(mode: ResetMode) {
    const id = repoId();
    const dialog = refDialogs.reset;
    refDialogs.reset = null;
    if (!id || !dialog) return;
    if (resetChoice(mode).destructive) {
      const go = await confirmation.ask({
        title: "Reset Hard",
        message:
          `Reset ${dialog.moving} to ${shortOid(dialog.oid)} and throw away the uncommitted changes to ` +
          "tracked files? They are stashed first, so Undo can bring them back.",
        confirm: "Reset Hard",
        warning: true,
      });
      if (!go) return;
    }
    await attempt("Could not reset", () => resetTo(id, dialog.oid, mode));
  }

  function pushSourceOf(at: Target): PushSource | null {
    if (at.ref?.kind === "branch" && at.branch) {
      return { kind: "branch", name: at.branch.name, upstream: at.branch.upstream };
    }
    if (at.ref?.kind === "tag") return { kind: "tag", name: at.ref.name, upstream: null };
    return null;
  }

  async function push(id: RepoId, remote: string, refspec: string) {
    await attempt("Could not push", () =>
      network.run(id, "Pushing", (onLine) => pushTo(id, remote, refspec, onLine)),
    );
  }

  async function pushTarget(id: RepoId, at: Target) {
    const source = pushSourceOf(at);
    const remote = source ? initialRemote(source, network.remotes, network.primary) : null;
    if (!source || !remote) return;
    await push(id, remote, pushRefspec(source, { mode: "tracked" }, remote, network.remotes));
  }

  async function pushUpToTarget(id: RepoId, at: Target) {
    const upstream = at.facts.upstream;
    const plan = at.oid && upstream ? pushUpTo(at.oid, upstream, network.remotes) : null;
    if (!plan) {
      errors.message("The branch's upstream is not on a configured remote.", "Could not push");
      return;
    }
    await push(id, plan.remote, plan.refspec);
  }

  async function sendPushTo(remote: string, refspec: string) {
    const id = repoId();
    refDialogs.push = null;
    if (id) await push(id, remote, refspec);
  }

  async function deleteTarget(id: RepoId, at: Target) {
    const ref = at.ref;
    if (!ref) return;
    if (ref.kind === "remote") {
      const remote = splitUpstream(ref.name, network.remotes)?.remote ?? ref.name.split("/")[0] ?? "origin";
      const go = await confirmation.ask({
        title: "Delete Remote Branch",
        message: `Delete ${ref.name} on ${remote}? This runs on the server, and Undo cannot reach it.`,
        confirm: "Delete",
        warning: true,
      });
      if (!go) return;
      await attempt("Could not delete the remote branch", async () => {
        // Someone else deleted it first: what was asked for is done (R-480).
        if ((await deleteRemoteBranch(id, remote, ref.name)) === "alreadyGone") {
          notices.inform("Branch already deleted", `${ref.name} was no longer on ${remote}.`);
        }
      });
      return;
    }
    const what = ref.kind === "tag" ? "tag" : "branch";
    const go = await confirmation.ask({
      title: `Delete ${ref.kind === "tag" ? "Tag" : "Branch"}`,
      message: `Delete ${what} ${ref.name}? Undo can bring it back.`,
      confirm: "Delete",
      warning: true,
    });
    if (!go) return;
    await attempt(`Could not delete the ${what}`, () =>
      ref.kind === "tag" ? deleteTag(id, ref.name) : deleteBranch(id, ref.name, false),
    );
  }

  async function renameTarget(id: RepoId, at: Target) {
    const ref = at.ref;
    if (!ref || ref.kind === "remote") return;
    // A tag is checked by git on the way in; only a branch has the rule here.
    const to = await prompt.ask({
      title: `Rename ${ref.name}`,
      label: "New name",
      value: ref.name,
      confirm: "Rename",
      validate:
        ref.kind === "tag"
          ? textProblem
          : (value) =>
              value.trim() === ref.name
                ? null
                : branchNameProblem(value, repository.localBranches.map((entry) => entry.name)),
    });
    if (to === null || to === ref.name) return;
    await attempt(`Could not rename the ${ref.kind === "tag" ? "tag" : "branch"}`, () =>
      ref.kind === "tag" ? renameTag(id, ref.name, to) : renameBranch(id, ref.name, to, false),
    );
  }

  async function renameStashTarget(id: RepoId, at: Target) {
    const stash = at.stash;
    if (!stash) return;
    const message = await prompt.ask({
      title: `Rename stash@{${stash.index}}`,
      label: "Message",
      value: stash.message,
      confirm: "Rename",
      validate: textProblem,
    });
    if (message === null || message === stash.message) return;
    await attempt("Could not rename the stash", () => renameStash(id, stash.index, message), afterMutation);
  }

  async function dropStashTarget(id: RepoId, at: Target) {
    const stash = at.stash;
    if (!stash) return;
    const go = await confirmation.ask({
      title: "Drop Stash",
      message: `Drop stash@{${stash.index}} (${stash.message})? Undo can bring it back.`,
      confirm: "Drop",
      warning: true,
    });
    if (go) await attempt("Could not drop the stash", () => stashes.drop(id, stash.index), afterMutation);
  }

  async function tickNode(node: RefNode) {
    if (refs.visible.has(node.id)) return;
    refs.set(new Set([...refs.visible, node.id]));
    await reloadGraph();
  }

  /** Ticks the row, then selects its commit and centres the graph on it. */
  async function reveal(id: RepoId, node: RefNode, oid: string) {
    await tickNode(node);
    stashView.clear();
    await commit.select(id, oid);
    graph.requestReveal(oid);
  }

  /** `to` becomes the selection, `from` stays marked; Files lists what differs. */
  async function compare(id: RepoId, from: string, to: string, node: RefNode | null) {
    if (node) await tickNode(node);
    stashView.clear();
    await commit.select(id, to);
    graph.requestReveal(to);
    await compareView.show(id, from, to);
  }

  async function toggle(node: RefNode) {
    const next = new Set(refs.visible);
    if (!next.delete(node.id)) next.add(node.id);
    refs.set(next);
    await reloadGraph();
  }

  async function worktreeAction(id: RepoId, name: string) {
    if (name === "wt-commit") commit.clear();
    else await worktree.load(id);
    await runWorkingTreeAction(name, worktree, {
      stage: (repo, paths) => worktree.stage(repo, paths),
      unstage: (repo, paths) => worktree.unstage(repo, paths),
      discard: (repo, paths) => worktree.discard(repo, paths),
      mutate,
      confirmDiscard: (paths) =>
        confirmation.ask({
          title: "Discard",
          message:
            `Discard the changes in ${paths.length === 1 ? paths[0] : `${paths.length} files`}? ` +
            "Staged changes and untracked files are kept. Undo can bring the changes back.",
          confirm: "Discard",
          warning: true,
        }),
      focusCommit: () => commitBox.focus(),
    });
  }
</script>

{#if confirmation.open}
  <ConfirmDialog
    title={confirmation.open.title}
    message={confirmation.open.message}
    confirm={confirmation.open.confirm}
    warning={confirmation.open.warning}
    onanswer={(yes) => confirmation.answer(yes)}
  />
{/if}

{#if refDialogs.tag}
  <AddTagDialog
    oid={refDialogs.tag.oid}
    subject={refDialogs.tag.subject}
    taken={(repository.current?.tags ?? []).map((tag) => tag.name)}
    check={checkTagName}
    onadd={createTagFrom}
    onclose={() => (refDialogs.tag = null)}
  />
{/if}

{#if refDialogs.push}
  <PushToDialog
    source={refDialogs.push}
    remotes={network.remotes}
    primary={network.primary}
    onpush={(remote, refspec) => void sendPushTo(remote, refspec)}
    onclose={() => (refDialogs.push = null)}
  />
{/if}

{#if refDialogs.reset}
  <ResetDialog
    moving={refDialogs.reset.moving}
    oid={refDialogs.reset.oid}
    subject={refDialogs.reset.subject}
    onreset={(mode) => void resetWith(mode)}
    onclose={() => (refDialogs.reset = null)}
  />
{/if}

{#if refDialogs.message}
  <EditMessageDialog
    oid={refDialogs.message.oid}
    message={refDialogs.message.message}
    onsave={(message) => void saveMessage(message)}
    onclose={() => (refDialogs.message = null)}
  />
{/if}

{#if refDialogs.author}
  <EditAuthorDialog
    oid={refDialogs.author.oid}
    name={refDialogs.author.name}
    email={refDialogs.author.email}
    onsave={(name, email) => void saveAuthor(name, email)}
    onclose={() => (refDialogs.author = null)}
  />
{/if}
