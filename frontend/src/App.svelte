<script lang="ts">
  import { ask, open as openFolderDialog } from "@tauri-apps/plugin-dialog";

  import BranchList from "$components/branch-tree/BranchList.svelte";
  import BlameView from "$components/diff/BlameView.svelte";
  import DiffView from "$components/diff/DiffView.svelte";
  import ConflictView from "$components/diff/ConflictView.svelte";
  import ImageDiff from "$components/diff/ImageDiff.svelte";
  import CommitBox from "$components/file-list/CommitBox.svelte";
  import SplitOffDialog from "$components/file-list/SplitOffDialog.svelte";
  import FileList from "$components/file-list/FileList.svelte";
  import CommitList from "$components/graph/CommitList.svelte";
  import GraphFilter from "$components/graph/GraphFilter.svelte";
  import RebaseEditor from "$components/graph/RebaseEditor.svelte";
  import RebaseProgressView from "$components/graph/RebaseProgressView.svelte";
  import DropMenu from "$components/layout/DropMenu.svelte";
  import Panel from "$components/layout/Panel.svelte";
  import CommandPalette from "$components/layout/CommandPalette.svelte";
  import SettingsPanel from "$components/layout/SettingsPanel.svelte";
  import HooksPanel from "$components/layout/HooksPanel.svelte";
  import FindObject from "$components/layout/FindObject.svelte";
  import GitErrorDialog from "$components/layout/GitErrorDialog.svelte";
  import OutputPanel from "$components/layout/OutputPanel.svelte";
  import StateBanner from "$components/layout/StateBanner.svelte";
  import Splitter from "$components/layout/Splitter.svelte";
  import StatusBar from "$components/layout/StatusBar.svelte";
  import Toolbar from "$components/layout/Toolbar.svelte";
  import LostCommits from "$components/branch-tree/LostCommits.svelte";
  import StashList from "$components/branch-tree/StashList.svelte";
  import TagList from "$components/branch-tree/TagList.svelte";
  import RepositoryList from "$components/repo-tree/RepositoryList.svelte";
  import SubmoduleList from "$components/repo-tree/SubmoduleList.svelte";
  import { shortOid } from "$lib/format";
  import { disabledIds, type PaletteCommand } from "$lib/palette";
  import { pullRequestUrl } from "$lib/pull-request";
  import { commitScope } from "$lib/commit-scope";
  import { dropActions, type DropAction, type DragPayload } from "$lib/drop-target";
  import { moveEntry } from "$lib/rebase-plan";
  import { stateBanner, type BannerAction } from "$lib/repo-state";
  import { PANELS, type PanelId } from "$lib/perspectives";
  import type { Settings } from "$lib/settings";
  import {
    checkout,
    deleteBranch,
    abortOperation,
    continueOperation,
    createBranch,
    createTag,
    deleteTag,
    getAppInfo,
    addToGitignore,
    cherryPick,
    deleteUntracked,
    findObject,
    interactiveRebase,
    isPublished,
    rebaseProgress,
    rebaseTodo,
    rollbackTo,
    splitOff,
    mergeInto,
    stageSelection,
    onMenuCommand,
    onRepoChanged,
    setMenuState,
    revertCommits,
    rebaseOnto,
    skipOperation,
    type AppInfo,
    type Branch,
    type RepoId,
    type Tag,
  } from "$lib/ipc";
  import { blame } from "$stores/blame.svelte";
  import { commit } from "$stores/commit.svelte";
  import { conflicts } from "$stores/conflicts.svelte";
  import { worktree } from "$stores/worktree.svelte";
  import { diff } from "$stores/diff.svelte";
  import { errors } from "$stores/errors.svelte";
  import { output } from "$stores/output.svelte";
  import { network } from "$stores/network.svelte";
  import { recovery } from "$stores/recovery.svelte";
  import { safety } from "$stores/safety.svelte";
  import { stashes } from "$stores/stashes.svelte";
  import { submodules } from "$stores/submodules.svelte";
  import { graph } from "$stores/graph.svelte";
  import { hooks } from "$stores/hooks.svelte";
  import { settings } from "$stores/settings.svelte";
  import { layout } from "$stores/layout.svelte";
  import { repository } from "$stores/repository.svelte";

  /** Settings the open diff was computed with: changing one has to re-run it. */
  const REDIFF: readonly (keyof Settings)[] = [
    "algorithm",
    "contextLines",
    "wordDiff",
    "detectMoves",
  ];

  const PANEL_TITLES: Record<PanelId, string> = {
    repositories: "Repositories",
    refs: "References",
    graph: "Graph",
    files: "Files",
    diff: "Diff",
  };

  /** Maximising acts on the panel the pointer last entered; there is no focus ring yet. */
  let focused = $state<PanelId>("graph");
  let info = $state<AppInfo | null>(null);
  let paletteOpen = $state(false);
  let settingsOpen = $state(false);
  let finderOpen = $state(false);
  let finderBusy = $state(false);
  let finderResults = $state.raw<import("$lib/ipc").Found[]>([]);
  let finderToken = 0;
  let recentCommands = $state<string[]>([]);
  let dropMenu = $state.raw<{
    actions: DropAction[];
    source: DragPayload;
    target: DragPayload;
    x: number;
    y: number;
  } | null>(null);
  let progress = $state.raw<import("$lib/ipc").RebaseProgress | null>(null);
  let rebaseOpen = $state(false);
  let rebaseBase = $state("");
  let rebasePlan = $state.raw<import("$lib/ipc").TodoEntry[]>([]);
  let rebaseBusy = $state(false);
  let splitOpen = $state(false);
  let splitPublished = $state(false);
  let splitBusy = $state(false);
  const pointer = { x: 0, y: 0 };
  let refFilter = $state("");
  let fileMask = $state("");

  $effect(() => {
    getAppInfo().then((result) => {
      info = result;
    });
    void settings.load().then(() => {
      diff.whitespace = settings.current.ignoreWhitespace;
    });
    void repository.restore().then(() => {
      const first = repository.openRepos[0];
      if (first) void activate(first.root);
    });
  });

  const fractions = $derived(layout.fractions);
  const shown = $derived({
    repositories: layout.visible("repositories"),
    refs: layout.visible("refs"),
    graph: layout.visible("graph"),
    files: layout.visible("files"),
    diff: layout.visible("diff"),
  });
  const leftColumn = $derived(shown.repositories || shown.refs);
  const topRow = $derived(shown.graph || shown.files);
  const repo = $derived(repository.current);
  const details = $derived(commit.details);
  const banner = $derived(repo ? stateBanner(repo.state, repo.indexLock) : null);
  const tracked = $derived(repository.localBranches.find((b) => b.isHead));
  const scope = $derived(commitScope(worktree.staged, fileMask));
  const prUrl = $derived.by(() => {
    const head = tracked?.name;
    const base = tracked?.upstream?.split("/").slice(1).join("/") ?? "main";
    if (!network.url || !head) return null;
    return pullRequestUrl(network.url, base, head, commit.details?.summary ?? head);
  });

  const onWorkingTree = $derived(repo !== undefined && repo !== null && commit.oid === null);

  $effect(() => {
    void commit.oid;
    diff.clear();
    blame.clear();
  });

  $effect(() => {
    const id = repository.current?.repo;
    if (id && commit.oid === null) void worktree.load(id);
  });

  // Every failure that carries raw Git output goes to the dialog; INV-05 says the user
  // sees exactly what Git said, not a summary of it.
  $effect(() => errors.report(worktree.error));
  $effect(() => errors.report(repository.error));
  $effect(() => errors.report(commit.error));
  $effect(() => errors.report(diff.error));
  $effect(() => errors.report(graph.error));
  $effect(() => errors.report(blame.error));
  $effect(() => errors.report(hooks.error));

  /** One place after every mutation: the reactive version fired on each loading toggle. */
  /** Refreshed with the rest of the state, so the stack follows Continue and Abort. */
  async function refreshProgress() {
    const id = repository.current?.repo;
    progress = id ? await rebaseProgress(id).catch(() => null) : null;
  }

  async function afterMutation(paths: string[] = []) {
    diff.dropIfAffected(paths);
    await repository.refreshStatus();
    const id = repository.current?.repo;
    await Promise.all([
      id ? stashes.refresh(id) : Promise.resolve(),
      id ? network.refresh(id) : Promise.resolve(),
      id ? recovery.refresh(id) : Promise.resolve(),
      id ? submodules.refresh(id) : Promise.resolve(),
      id ? conflicts.refresh(id) : Promise.resolve(),
      output.refreshProblems(),
      safety.refresh(),
      refreshProgress(),
      output.open ? output.refresh() : Promise.resolve(),
    ]);
  }

  const palette = $derived.by<PaletteCommand[]>(() => {
    const open = repo !== undefined && repo !== null;
    const noRepo = open ? undefined : "No repository is open";
    const noRemote = network.primary ? undefined : "This repository has no remote";
    const nothingStaged = worktree.staged.length > 0 ? undefined : "Nothing is staged";

    return [
      { id: "open", title: "Open Repository…", run: () => void pickRepository() },
      { id: "fetch", title: "Fetch", unavailable: noRepo ?? noRemote, run: () => void runNetwork("fetch") },
      { id: "pull", title: "Pull", unavailable: noRepo ?? noRemote, run: () => void runNetwork("pull") },
      { id: "push", title: "Push", unavailable: noRepo ?? noRemote, run: () => void runNetwork("push") },
      { id: "stash", title: "Stash All", synonyms: ["shelve"], unavailable: noRepo, run: () => void stashAll() },
      { id: "tag", title: "Create Tag", unavailable: noRepo, run: () => void tagHead() },
      { id: "commit", title: "Commit Staged", unavailable: noRepo ?? nothingStaged, run: () => {} },
      {
        id: "undo",
        title: "Undo Last Operation",
        unavailable: safety.last ? undefined : "Nothing to undo",
        run: () => void undo(),
      },
      { id: "output", title: "Toggle Output Panel", shortcut: "Ctrl+Shift+7", run: () => output.toggle() },
      {
        id: "rebase-i",
        title: "Rebase Commits After This One…",
        synonyms: ["interactive rebase", "squash", "reorder"],
        unavailable: commit.oid ? undefined : "Select a commit first",
        run: () => void openRebase(),
      },
      {
        id: "split-off",
        title: "Split Off Files…",
        synonyms: ["split commit", "surgery"],
        unavailable: commit.oid ? undefined : "Select a commit first",
        run: () => void openSplit(),
      },
      {
        id: "rollback",
        title: "Roll Back Tree To This Commit",
        synonyms: ["restore", "revert files"],
        unavailable: commit.oid ? undefined : "Select a commit first",
        run: () => void rollbackFiles([]),
      },
      {
        id: "close",
        title: "Close Repository",
        shortcut: "Ctrl+W",
        unavailable: noRepo,
        run: () => void closeCurrent(),
      },
      { id: "refresh", title: "Refresh", shortcut: "F5", unavailable: noRepo, run: () => void repository.refresh() },
      {
        id: "branch",
        title: "New Branch…",
        unavailable: noRepo,
        run: () => void runBannerAction("createBranch"),
      },
      { id: "reset-layout", title: "Reset Perspective", run: () => layout.reset() },
      {
        id: "hooks",
        title: "Manage Hooks…",
        synonyms: ["pre-commit", "git hooks"],
        unavailable: noRepo,
        run: () => {
          const id = repository.current?.repo;
          if (id) void hooks.show(id);
        },
      },
      {
        id: "perspective-main",
        title: "Perspective: Main",
        run: () => layout.switch("main"),
      },
      {
        id: "perspective-review",
        title: "Perspective: Review",
        run: () => layout.switch("review"),
      },
      {
        id: "maximize-panel",
        title: "Maximise Panel",
        shortcut: "Shift+F11",
        synonyms: ["zoom", "full screen panel"],
        run: () => layout.toggleMaximized(focused),
      },
      ...PANELS.map((panel) => ({
        id: `panel-${panel}`,
        title: `Toggle ${PANEL_TITLES[panel]} Panel`,
        run: () => layout.togglePanel(panel),
      })),
      {
        id: "palette",
        title: "Find Command",
        shortcut: "Ctrl+Shift+P",
        run: () => (paletteOpen = true),
      },
      {
        id: "about",
        title: "About Cogit",
        run: () => window.alert(`Cogit ${info?.version ?? ""}

Log: ${info?.logPath ?? ""}`),
      },
      {
        id: "settings",
        title: "Settings",
        shortcut: "Ctrl+,",
        synonyms: ["preferences", "options"],
        run: () => (settingsOpen = true),
      },
      {
        id: "find",
        title: "Find Object",
        shortcut: "Ctrl+P",
        synonyms: ["goto", "jump"],
        unavailable: noRepo,
        run: () => (finderOpen = true),
      },
      {
        id: "pr",
        title: "Create Pull Request",
        synonyms: ["merge request", "pr", "mr"],
        unavailable: prUrl ? undefined : "No GitHub, GitLab or Bitbucket remote",
        run: () => void openPullRequest(),
      },
      {
        id: "copy-pr",
        title: "Copy Pull Request Link",
        unavailable: prUrl ? undefined : "No GitHub, GitLab or Bitbucket remote",
        run: () => {
          if (prUrl) void import("@tauri-apps/plugin-clipboard-manager").then((m) => m.writeText(prUrl));
        },
      },
      {
        id: "blame",
        title: "Blame This File",
        unavailable: diff.path ? undefined : "No file is open in the Diff panel",
        run: () => void showBlame(),
      },
      {
        id: "abort",
        title: "Abort Operation In Progress",
        unavailable: banner?.actions.includes("abort") ? undefined : "Nothing is in progress",
        run: () => void runBannerAction("abort"),
      },
    ];
  });

  async function openPullRequest() {
    if (!prUrl) return;
    if (tracked && tracked.ahead > 0) {
      const push = await ask(
        `${tracked.name} has ${tracked.ahead} commit(s) the remote has not seen. Push first?`,
        { title: "Create pull request", kind: "info" },
      );
      if (push) await runNetwork("push");
    }
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(prUrl);
  }

  async function runFind(text: string) {
    const id = repository.current?.repo;
    const token = ++finderToken;
    if (!id || text.trim() === "") {
      finderResults = [];
      return;
    }
    finderBusy = true;
    try {
      const found = await findObject(id, text);
      if (token === finderToken) finderResults = found;
    } catch (err) {
      errors.report(err as never);
    } finally {
      if (token === finderToken) finderBusy = false;
    }
  }

  function pickFound(item: import("$lib/ipc").Found) {
    finderOpen = false;
    const id = repository.current?.repo;
    if (!id) return;
    if (item.kind === "commit") void commit.select(id, item.oid);
    if (item.kind === "branch") void switchTo({ name: item.label } as Branch);
    if (item.kind === "tag" && item.oid) void commit.select(id, item.oid);
    if (item.kind === "file" && commit.oid) openDiff(item.label);
  }

  function runCommand(command: PaletteCommand) {
    paletteOpen = false;
    recentCommands = [command.id, ...recentCommands.filter((id) => id !== command.id)].slice(0, 8);
    command.run();
  }

  // Accords that appear in the native menu are owned by it: handling them here too
  // would run the command twice for one keypress.
  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      settingsOpen = false;
      paletteOpen = false;
      finderOpen = false;
    }
  }

  async function changeSetting<K extends keyof Settings>(key: K, value: Settings[K]) {
    await settings.set(key, value);
    const id = repository.current?.repo;
    if (key === "ignoreWhitespace" && id) {
      await diff.setWhitespace(id, settings.current.ignoreWhitespace);
    } else if (REDIFF.includes(key) && id && diff.spec && diff.path) {
      await diff.load(id, diff.spec, diff.path);
    }
  }

  // The watcher is the only way Cogit learns about work done in a terminal alongside it.
  $effect(() => {
    const unlisten = onRepoChanged((change) => {
      const id = repository.current?.repo;
      if (!id || id.valueOf() !== change.repo.valueOf()) return;
      // Only a ref move needs the full re-read; an index or worktree change moves counters.
      const movedRefs = change.kind === "head" || change.kind === "refs";
      void (movedRefs ? repository.refresh() : repository.refreshStatus());
      if (commit.oid === null) void worktree.load(id);
      void afterMutation();
      if (movedRefs) void graph.load(id, graph.query);
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  });

  function filterGraph(query: import("$lib/ipc").CommitQuery) {
    const id = repository.current?.repo;
    if (!id) return;
    commit.clear();
    diff.clear();
    void graph.load(id, query);
  }

  async function stage(paths: string[]) {
    const id = repository.current?.repo;
    if (!id) return;
    await worktree.stage(id, paths);
    await afterMutation(paths);
  }

  async function unstage(paths: string[]) {
    const id = repository.current?.repo;
    if (!id) return;
    await worktree.unstage(id, paths);
    await afterMutation(paths);
  }

  async function ignore(paths: string[]) {
    const id = repository.current?.repo;
    if (!id) return;
    try {
      await addToGitignore(id, paths);
    } catch (err) {
      errors.report(err as never);
      return;
    }
    await worktree.load(id);
    await afterMutation(paths);
  }

  async function discard(paths: string[]) {
    const id = repository.current?.repo;
    if (!id) return;
    const what = paths.length === 1 ? paths[0] : `${paths.length} files`;
    const confirmed = await ask(`Discard changes in ${what}? Undo can bring them back.`, {
      title: "Discard changes",
      kind: "warning",
    });
    if (!confirmed) return;
    await worktree.discard(id, paths);
    await afterMutation(paths);
  }

  async function deleteFromDisk(paths: string[]) {
    const id = repository.current?.repo;
    if (!id) return;
    const what = paths.length === 1 ? paths[0] : `${paths.length} untracked paths`;
    const confirmed = await ask(
      `Delete ${what} from disk? Untracked files are not in Git, so this cannot be undone.`,
      { title: "Delete from disk", kind: "warning" },
    );
    if (!confirmed) return;
    try {
      await deleteUntracked(id, paths);
    } catch (err) {
      errors.report(err as never);
      return;
    }
    await worktree.load(id);
    await afterMutation(paths);
  }

  async function commitStaged(message: string, amend: boolean, noVerify: boolean) {
    const id = repository.current?.repo;
    if (!id) return;
    if (scope.paths) {
      const listed = scope.paths.join("\n");
      const confirmed = await ask(
        `Commit only these ${scope.paths.length} file(s)?\n\n${listed}\n\n${scope.warning}.`,
        { title: "Commit what you see", kind: "warning" },
      );
      if (!confirmed) return;
    }
    await worktree.commit(id, message, amend, noVerify, scope.paths ?? []);
    if (worktree.error) return;
    diff.clear();
    await repository.refresh();
    await afterMutation();
    void graph.load(id, graph.query);
  }

  async function afterRefChange() {
    const id = repository.current?.repo;
    if (!id) return;
    commit.clear();
    diff.clear();
    await repository.refresh();
    await worktree.load(id);
    await afterMutation();
    void graph.load(id, graph.query);
  }

  async function switchTo(branch: Branch) {
    const id = repository.current?.repo;
    if (!id) return;
    try {
      await checkout(id, { kind: "branch", name: branch.name });
    } catch (err) {
      errors.report(err as never);
      return;
    }
    await afterRefChange();
  }

  async function removeBranch(branch: Branch) {
    const id = repository.current?.repo;
    if (!id) return;
    const confirmed = await ask(`Delete branch ${branch.name}?`, {
      title: "Delete branch",
      kind: "warning",
    });
    if (!confirmed) return;
    try {
      await deleteBranch(id, branch.name, false);
    } catch (err) {
      errors.report(err as never);
      return;
    }
    await afterRefChange();
  }

  async function undo() {
    const id = repository.current?.repo;
    if (!id) return;
    try {
      await safety.undo(id);
    } catch (err) {
      errors.report(err as never);
      return;
    }
    await afterRefChange();
  }

  async function showBlame() {
    const id = repository.current?.repo;
    const path = diff.path;
    if (!id || !path) return;
    await blame.show(id, path, commit.oid ?? "HEAD");
  }

  async function stageLines(selected: ReadonlySet<string>, reverse: boolean) {
    const id = repository.current?.repo;
    const path = diff.path;
    if (!id || !path) return;
    const { splitSelection } = await import("$lib/selection");
    const { deletes, inserts } = splitSelection(selected);

    try {
      await stageSelection(
        id,
        {
          path,
          hunks: diff.hunks,
          selectedDeletes: deletes,
          selectedInserts: inserts,
          lineEnding: diff.diff?.kind === "text" ? diff.diff.eol.old : "lf",
          noTrailingNewline: false,
        },
        reverse,
      );
    } catch (err) {
      errors.report(err as never);
      return;
    }
    await worktree.load(id);
    await afterMutation([path]);
  }

  async function refreshSubmodule(module: import("$lib/ipc").Submodule) {
    const id = repository.current?.repo;
    if (!id) return;
    try {
      await submodules.update(id, module.path, module.state === "notInitialised");
    } catch (err) {
      errors.report(err as never);
    }
    await afterMutation();
  }

  async function recoverCommit(lost: import("$lib/ipc").CommitRow) {
    const id = repository.current?.repo;
    if (!id) return;
    const name = window.prompt("Branch name for the recovered commit:", "recovered");
    if (!name) return;
    try {
      await createBranch(id, name, lost.oid, false);
    } catch (err) {
      errors.report(err as never);
      return;
    }
    await afterRefChange();
  }

  async function replaySelected(kind: "cherryPick" | "revert") {
    const id = repository.current?.repo;
    const oid = commit.oid;
    if (!id || !oid) return;
    const verb = kind === "cherryPick" ? "Cherry-pick" : "Revert";
    const confirmed = await ask(`${verb} ${oid.slice(0, 7)} onto the current branch?`, {
      title: verb,
      kind: "warning",
    });
    if (!confirmed) return;
    try {
      if (kind === "cherryPick") await cherryPick(id, [oid]);
      else await revertCommits(id, [oid]);
    } catch (err) {
      errors.report(err as never);
    }
    await afterRefChange();
  }

  async function rebaseOntoBranch(branch: Branch) {
    const id = repository.current?.repo;
    if (!id) return;
    try {
      await rebaseOnto(id, { onto: branch.name, autostash: true });
    } catch (err) {
      errors.report(err as never);
    }
    await afterRefChange();
  }

  async function mergeBranch(branch: Branch) {
    const id = repository.current?.repo;
    if (!id) return;
    try {
      await mergeInto(id, {
        source: branch.name,
        noFastForward: false,
        squash: false,
        message: null,
      });
    } catch (err) {
      errors.report(err as never);
      await afterRefChange();
      return;
    }
    await afterRefChange();
  }

  async function runNetwork(kind: "fetch" | "pull" | "push") {
    const id = repository.current?.repo;
    const remote = network.primary;
    if (!id) return;
    if (!remote) {
      errors.report({ message: "This repository has no remote." } as never);
      return;
    }
    try {
      if (kind === "fetch") await network.fetch(id, remote);
      if (kind === "pull") await network.pull(id, remote, true);
      if (kind === "push") await network.push(id, remote, false);
    } catch (err) {
      errors.report(err as never);
      await afterMutation();
      return;
    }
    await afterRefChange();
  }

  async function tagHead() {
    const id = repository.current?.repo;
    if (!id) return;
    const name = window.prompt("Tag name for the current commit:");
    if (!name) return;
    const message = window.prompt("Message (leave empty for a lightweight tag):", "");
    try {
      await createTag(id, {
        name,
        target: null,
        message: message ? message : null,
        force: false,
      });
    } catch (err) {
      errors.report(err as never);
      return;
    }
    await afterRefChange();
  }

  async function removeTag(tag: Tag) {
    const id = repository.current?.repo;
    if (!id) return;
    const confirmed = await ask(`Delete tag ${tag.name}? Undo can bring it back.`, {
      title: "Delete tag",
      kind: "warning",
    });
    if (!confirmed) return;
    try {
      await deleteTag(id, tag.name);
    } catch (err) {
      errors.report(err as never);
      return;
    }
    await afterRefChange();
  }

  async function checkoutTag(tag: Tag) {
    const id = repository.current?.repo;
    if (!id) return;
    try {
      await checkout(id, { kind: "commit", oid: tag.oid });
    } catch (err) {
      errors.report(err as never);
      return;
    }
    await afterRefChange();
  }

  async function stashAll() {
    const id = repository.current?.repo;
    if (!id) return;
    const message = window.prompt("Stash message:", "");
    if (message === null) return;
    try {
      await stashes.push(id, message, true);
    } catch (err) {
      errors.report(err as never);
      return;
    }
    await afterRefChange();
  }

  async function applyStash(index: number, pop: boolean) {
    const id = repository.current?.repo;
    if (!id) return;
    try {
      await stashes.apply(id, index, pop);
    } catch (err) {
      errors.report(err as never);
      return;
    }
    await afterRefChange();
  }

  async function dropStash(index: number) {
    const id = repository.current?.repo;
    if (!id) return;
    const confirmed = await ask(`Drop stash@{${index}}? Undo can bring it back.`, {
      title: "Drop stash",
      kind: "warning",
    });
    if (!confirmed) return;
    try {
      await stashes.drop(id, index);
    } catch (err) {
      errors.report(err as never);
      return;
    }
    await afterMutation();
  }

  async function runBannerAction(action: BannerAction) {
    const id = repository.current?.repo;
    if (!id) return;
    try {
      if (action === "abort") await abortOperation(id);
      if (action === "continue") await continueOperation(id);
      if (action === "skip") await skipOperation(id);
      if (action === "createBranch") {
        const name = window.prompt("Name for the new branch at this commit:");
        if (!name) return;
        await createBranch(id, name, null, true);
      }
    } catch (err) {
      errors.report(err as never);
      return;
    }
    await afterRefChange();
  }

  function openDiff(path: string) {
    const id = repository.current?.repo;
    const oid = commit.oid;
    if (id && oid) void diff.load(id, { kind: "commitVsParent", oid }, path);
  }

  function openStagedDiff(path: string) {
    const id = repository.current?.repo;
    if (id) void diff.load(id, { kind: "indexVsHead" }, path);
  }

  function openWorktreeDiff(path: string) {
    const id = repository.current?.repo;
    if (!id) return;
    // A conflicted file has three sides; a two-sided diff of it says nothing useful.
    if (conflicts.paths.includes(path)) {
      diff.clear();
      void conflicts.open(id, path);
      return;
    }
    conflicts.close();
    void diff.load(id, { kind: "workTreeVsIndex" }, path);
  }

  async function activate(root: string) {
    commit.clear();
    diff.clear();
    blame.clear();
    worktree.clear();
    stashes.clear();
    network.clear();
    recovery.clear();
    submodules.clear();
    conflicts.clear();
    await repository.open(root);
    const opened = repository.current;
    if (opened) {
      void graph.load(opened.repo);
      await repository.refreshList();
      await afterMutation();
    } else {
      graph.clear();
    }
  }

  async function closeOne(entry: import("$lib/ipc").RepoOverview) {
    await repository.closeOne(entry.repo);
    const next = repository.openRepos[0];
    if (next) await activate(next.root);
    else graph.clear();
  }

  /** Restores a past version into the working tree; HEAD stays where it is. */
  async function rollbackFiles(paths: string[]) {
    const id = repository.current?.repo;
    const rev = commit.oid;
    if (!id || !rev) return;
    const what = paths.length > 0 ? paths.join(", ") : "every file";
    const go = await ask(`Restore ${what} as it was in ${shortOid(rev)}?`, {
      title: "Roll back",
      kind: "warning",
    });
    if (!go) return;
    try {
      await rollbackTo(id, rev, paths);
    } catch (err) {
      errors.report(err as never);
    }
    await afterMutation(paths);
  }

  /** Opens the plan editor for everything after the selected commit. */
  /** A drop never acts on its own: the user picks from the menu it opens. */
  function onBranchDrop(sourceName: string, target: Branch) {
    const source = repository.localBranches.find((b) => b.name === sourceName);
    if (!source) return;
    const canFastForward = target.isHead && source.oid !== target.oid;
    const payload = { kind: "branch" as const, id: sourceName };
    const onto = { kind: "branch" as const, id: target.name };
    const actions = dropActions(payload, onto, canFastForward);
    if (actions.length === 0) return;
    dropMenu = { actions, source: payload, target: onto, x: pointer.x, y: pointer.y };
  }

  async function runDropAction(action: DropAction) {
    const menu = dropMenu;
    dropMenu = null;
    const id = repository.current?.repo;
    if (!menu || !id) return;

    if (menu.source.kind === "commit") {
      await replan(action, menu.source.id, menu.target.id);
      return;
    }

    if (action.destructive) {
      const go = await ask(`${action.title}. This rewrites history. Continue?`, {
        title: "Rewrite history",
        kind: "warning",
      });
      if (!go) return;
    }

    try {
      if (action.id === "merge") {
        await mergeInto(id, {
          source: menu.source.id,
          noFastForward: false,
          squash: false,
          message: null,
        });
      } else if (action.id === "rebase") {
        await rebaseOnto(id, { onto: menu.target.id, autostash: true });
      } else if (action.id === "fastForward") {
        await mergeInto(id, {
          source: menu.source.id,
          noFastForward: false,
          squash: false,
          message: null,
        });
      }
    } catch (err) {
      errors.report(err as never);
    }
    await afterRefChange();
  }

  function onCommitDrop(sourceOid: string, targetOid: string) {
    const source = { kind: "commit" as const, id: sourceOid };
    const target = { kind: "commit" as const, id: targetOid };
    const actions = dropActions(source, target, false);
    if (actions.length === 0) return;
    dropMenu = { actions, source, target, x: pointer.x, y: pointer.y };
  }

  /** Squash and reorder are both one interactive rebase with a two-line plan. */
  async function replan(action: DropAction, source: string, target: string) {
    const id = repository.current?.repo;
    if (!id) return;
    const base = await mergeBaseOf(id, source, target);
    if (base === null) return;

    let plan;
    try {
      plan = await rebaseTodo(id, base);
    } catch (err) {
      errors.report(err as never);
      return;
    }

    const from = plan.findIndex((entry) => entry.oid === source);
    const onto = plan.findIndex((entry) => entry.oid === target);
    if (from < 0 || onto < 0) {
      errors.message("Both commits have to be on the current branch above their common parent.");
      return;
    }

    let moved = moveEntry(plan, from, onto < from ? onto + 1 : onto);
    if (action.id === "squash") {
      moved = moved.map((entry) =>
        entry.oid === source ? { ...entry, action: "squash" as const } : entry,
      );
    }

    rebaseBase = base;
    rebasePlan = moved;
    splitPublished = await isPublished(id, base).catch(() => false);
    rebaseOpen = true;
  }

  /** The parent of the older of the two, so the plan covers both. */
  async function mergeBaseOf(id: RepoId, a: string, b: string): Promise<string | null> {
    try {
      const plan = await rebaseTodo(id, `${a}^`);
      if (plan.some((entry) => entry.oid === b)) return `${a}^`;
      return `${b}^`;
    } catch {
      errors.message("Both commits have to be on the current branch.");
      return null;
    }
  }

  async function openRebase() {
    const id = repository.current?.repo;
    const rev = commit.oid;
    if (!id || !rev) return;
    try {
      rebasePlan = await rebaseTodo(id, rev);
    } catch (err) {
      errors.report(err as never);
      return;
    }
    if (rebasePlan.length === 0) {
      errors.message("This commit is the tip; there is nothing after it to rebase.");
      return;
    }
    rebaseBase = rev;
    splitPublished = await isPublished(id, rev).catch(() => false);
    rebaseOpen = true;
  }

  async function runRebase() {
    const id = repository.current?.repo;
    if (!id) return;
    rebaseBusy = true;
    try {
      await interactiveRebase(id, rebaseBase, rebasePlan);
      rebaseOpen = false;
      commit.clear();
    } catch (err) {
      errors.report(err as never);
    } finally {
      rebaseBusy = false;
    }
    await afterRefChange();
  }

  async function openSplit() {
    const id = repository.current?.repo;
    const rev = commit.oid;
    if (!id || !rev) return;
    splitPublished = await isPublished(id, rev).catch(() => false);
    splitOpen = true;
  }

  async function runSplit(paths: string[], message: string, splitFirst: boolean) {
    const id = repository.current?.repo;
    const rev = commit.oid;
    if (!id || !rev) return;
    splitBusy = true;
    try {
      await splitOff(id, rev, paths, message, splitFirst);
      splitOpen = false;
      await repository.refresh();
      commit.clear();
    } catch (err) {
      errors.report(err as never);
    } finally {
      splitBusy = false;
    }
    await afterMutation();
  }

  async function pickRepository() {
    const picked = await openFolderDialog({ directory: true, title: "Open Repository" });
    if (typeof picked !== "string") return;
    await activate(picked);
  }

  async function closeCurrent() {
    const id = repository.current?.repo;
    if (!id) return;
    commit.clear();
    diff.clear();
    blame.clear();
    await repository.closeOne(id);
  }

  $effect(() => {
    const pending = onMenuCommand((id) => {
      const command = palette.find((entry) => entry.id === id);
      if (command && !command.unavailable) runCommand(command);
    });
    return () => void pending.then((unlisten) => unlisten());
  });

  // The native menu is not reactive, so the derived availability is pushed to it.
  $effect(() => {
    void setMenuState(disabledIds(palette)).catch(() => {});
  });
</script>

<svelte:window
  {onkeydown}
  ondragover={(event) => {
    pointer.x = event.clientX;
    pointer.y = event.clientY;
  }}
/>

<div class="app">
  <Toolbar
    busy={network.running ?? (repository.busy ? "Opening repository…" : undefined)}
    undoable={safety.last?.description}
    onundo={undo}
    handlers={{
      stash: stashAll,
      tag: tagHead,
      pull: () => void runNetwork("pull"),
      push: () => void runNetwork("push"),
      sync: () => void runNetwork("fetch"),
    }}
  />

  {#if banner}
    <StateBanner {banner} busy={repository.busy} onaction={runBannerAction} />
  {/if}

  <div class="workspace">
    {#if leftColumn}
    <div
      class="left-column"
      style:flex={shown.diff || topRow ? `0 0 ${fractions.leftColumn * 100}%` : "1 1 auto"}
    >
      {#if shown.repositories}
      <div
        class="pane"
        class:grow={!shown.refs}
        style:flex={shown.refs ? `0 0 ${fractions.repositories * 100}%` : undefined}
        role="region"
        aria-label={PANEL_TITLES.repositories}
        onpointerenter={() => (focused = "repositories")}
      >
        <Panel title="Repositories" count={repository.openRepos.length}>
          <RepositoryList
            onopen={pickRepository}
            onselect={(entry) => void activate(entry.root)}
            onclose={(entry) => void closeOne(entry)}
          />
          <SubmoduleList
            modules={submodules.entries}
            onopen={(module) => void activate(`${repo?.root ?? ""}/${module.path}`)}
            onupdate={(module) => void refreshSubmodule(module)}
          />
        </Panel>
      </div>
      {/if}
      {#if shown.repositories && shown.refs}
      <Splitter
        direction="horizontal"
        value={fractions.repositories}
        label="Resize repositories panel"
        onchange={(d) => layout.nudge("repositories", d)}
        onreset={() => layout.resetOne("repositories")}
      />
      {/if}
      {#if shown.refs}
      <div class="pane grow" role="region"
        aria-label={PANEL_TITLES.refs}
        onpointerenter={() => (focused = "refs")}>
        <Panel
          title="References"
          count={repo?.branches.length}
          empty={repo ? undefined : "Open a repository to see its branches."}
        >
          {#snippet actions()}
            {#if repo}
              <input
                class="ref-filter"
                type="search"
                bind:value={refFilter}
                placeholder="Filter refs"
                aria-label="Filter references"
              />
            {/if}
          {/snippet}
          {#if repo}
            {#if repo.branches.length === 0}
              <p class="note">No branches yet — the first commit creates one.</p>
            {:else}
              <BranchList
                title="Local Branches"
                branches={repository.localBranches}
                oncheckout={switchTo}
                ondelete={removeBranch}
                onmerge={mergeBranch}
                onrebase={rebaseOntoBranch}
                ondrop={onBranchDrop}
                filter={refFilter}
              />
              <BranchList
                title="Remote"
                branches={repository.remoteBranches}
                oncheckout={switchTo}
                filter={refFilter}
              />
              <TagList tags={repo.tags} oncheckout={checkoutTag} ondelete={removeTag} />
              <StashList stashes={stashes.entries} onapply={applyStash} ondrop={dropStash} />
              <LostCommits commits={recovery.lost} onrestore={recoverCommit} />
            {/if}
          {/if}
        </Panel>
      </div>
      {/if}
    </div>
    {/if}

    {#if leftColumn && (topRow || shown.diff)}
    <Splitter
      direction="vertical"
      value={fractions.leftColumn}
      label="Resize left column"
      onchange={(d) => layout.nudge("leftColumn", d)}
      onreset={() => layout.resetOne("leftColumn")}
    />
    {/if}

    {#if topRow || shown.diff}
    <div class="right-area">
      {#if topRow}
      <div
        class="top-row"
        style:flex={shown.diff ? `0 0 ${fractions.topRow * 100}%` : "1 1 auto"}
      >
        {#if shown.graph}
        <div
          class="pane"
          class:grow={!shown.files}
          style:flex={shown.files ? `0 0 ${fractions.graph * 100}%` : undefined}
          role="region"
        aria-label={PANEL_TITLES.graph}
        onpointerenter={() => (focused = "graph")}
        >
          <Panel title="Graph &amp; History" count={graph.rows.length}>
            {#snippet actions()}
              {#if repo}
                <GraphFilter onchange={filterGraph} matches={graph.rows.length} />
              {/if}
            {/snippet}
            {#if repo}
              {#if progress}
                <RebaseProgressView
                  {progress}
                  changes={worktree.total}
                  staged={worktree.staged.length}
                />
              {/if}
              <CommitList ondrop={onCommitDrop} />
            {:else}
              <p class="note">Open a repository to see its history.</p>
            {/if}
          </Panel>
        </div>
        {/if}
        {#if shown.graph && shown.files}
        <Splitter
          direction="vertical"
          value={fractions.graph}
          label="Resize graph panel"
          onchange={(d) => layout.nudge("graph", d)}
          onreset={() => layout.resetOne("graph")}
        />
        {/if}
        {#if shown.files}
        <div class="pane grow" role="region"
        aria-label={PANEL_TITLES.files}
        onpointerenter={() => (focused = "files")}>
          <Panel title="Files" count={onWorkingTree ? worktree.total : commit.files.length}>
            <div class="files">
            {#if onWorkingTree}
              <FileList
                sections={[
                  {
                    title: "Staged",
                    files: worktree.staged,
                    onselect: openStagedDiff,
                    actions: [{ label: "Unstage", title: "Unstage", run: unstage }],
                  },
                  {
                    title: "Unstaged",
                    files: worktree.unstaged,
                    onselect: openWorktreeDiff,
                    actions: [
                      { label: "Stage", title: "Stage", run: stage },
                      { label: "Discard", title: "Discard changes", run: discard },
                      { label: "Ignore", title: "Add to .gitignore", run: ignore },
                      { label: "Delete", title: "Delete from disk", run: deleteFromDisk },
                    ],
                  },
                ]}
                empty="The working tree is clean."
                selected={diff.path}
                onmask={(mask) => (fileMask = mask)}
              />
              <CommitBox
                {scope}
                stagedCount={worktree.staged.length}
                busy={worktree.loading}
                draftKey={`cogit:draft:${repo?.root ?? ""}`}
                oncommit={commitStaged}
              />
            {:else}
              <FileList
                sections={[{ files: commit.files }]}
                empty="Select a commit to see the files it changed."
                selected={diff.path}
                onselect={openDiff}
              />
            {/if}
            </div>
          </Panel>
        </div>
        {/if}
      </div>
      {/if}

      {#if topRow && shown.diff}
      <Splitter
        direction="horizontal"
        value={fractions.topRow}
        label="Resize diff panel"
        onchange={(d) => layout.nudge("topRow", d)}
        onreset={() => layout.resetOne("topRow")}
      />
      {/if}

      {#if shown.diff}
      <div class="pane grow" role="region"
        aria-label={PANEL_TITLES.diff}
        onpointerenter={() => (focused = "diff")}>
        <Panel title="Diff">
          {#if conflicts.path}
            <ConflictView
              path={conflicts.path}
              base={conflicts.base}
              ours={conflicts.ours}
              theirs={conflicts.theirs}
              onresolve={(side) => {
                const id = repository.current?.repo;
                if (id) void conflicts.take(id, side).then(() => afterMutation());
              }}
              onresolveText={(text) => {
                const id = repository.current?.repo;
                if (id) void conflicts.write(id, text).then(() => afterMutation());
              }}
            />
          {:else if blame.path}
            <BlameView
              lines={blame.lines}
              path={blame.path}
              onselect={(oid) => {
                blame.clear();
                const id = repository.current?.repo;
                if (id) void commit.select(id, oid);
              }}
            />
          {:else if diff.error}
            <p class="error detail">{diff.error.message}</p>
          {:else if diff.diff?.kind === "image"}
            <ImageDiff
              before={diff.images[0]}
              after={diff.images[1]}
              oldSize={diff.diff.oldSize}
              newSize={diff.diff.newSize}
              mime={diff.diff.mime}
            />
          {:else if diff.diff && diff.path}
            <DiffView
              diff={diff.diff}
              path={diff.path}
              stageable={diff.stageable}
              onstage={(selected, reverse) => void stageLines(selected, reverse)}
              onblame={() => void showBlame()}
              whitespace={diff.whitespace}
              onwhitespace={(mode) => {
                const id = repository.current?.repo;
                if (id) void diff.setWhitespace(id, mode);
              }}
            />
          {:else}
          <div class="detail">
            {#if repository.error}
              <p class="error">{repository.error.message}</p>
              {#if repository.error.isCommandFailure && repository.error.detail.kind === "command"}
                <pre class="raw">{repository.error.detail.data.stderr}</pre>
              {/if}
            {:else if commit.error}
              <p class="error">{commit.error.message}</p>
            {:else if details}
              <p class="subject">{details.summary}</p>
              {#if details.body}<pre class="body">{details.body}</pre>{/if}
              <dl>
                <dt>Commit</dt>
                <dd class="mono">{details.oid}</dd>
                <dt>Author</dt>
                <dd>
                  {details.author.name} &lt;{details.author.email}&gt; ·
                  {settings.formatDate(details.author.timestamp, details.author.tzOffsetMinutes)}
                </dd>
                <dt>Parents</dt>
                <dd class="mono tabular">
                  {details.parents.length === 0
                    ? "none (root commit)"
                    : details.parents.map(shortOid).join(", ")}
                </dd>
              </dl>
              <div class="commit-actions">
                <button type="button" onclick={() => void replaySelected("cherryPick")}
                  >Cherry-pick</button
                >
                <button type="button" onclick={() => void replaySelected("revert")}>Revert</button>
                <button type="button" onclick={() => void openSplit()}>Split Off…</button>
                <button type="button" onclick={() => void openRebase()}>Rebase…</button>
                <button type="button" onclick={() => void rollbackFiles([])}>Roll Back Tree</button>
              </div>
            {:else if repo}
              <dl>
                <dt>Repository</dt>
                <dd class="mono">{repo.root}</dd>
                <dt>HEAD</dt>
                <dd class="mono">{repository.headLabel}</dd>
                <dt>Branches</dt>
                <dd class="mono tabular">
                  {repository.localBranches.length} local, {repository.remoteBranches.length} remote
                </dd>
              </dl>
              <p class="muted">Select a commit to see what it changed.</p>
            {:else}
              <p class="muted">Open a repository to begin.</p>
            {/if}
          </div>
          {/if}
        </Panel>
      </div>
      {/if}
    </div>
    {/if}
  </div>

  {#if output.open}
    <OutputPanel />
  {/if}

  {#if finderOpen}
    <FindObject
      results={finderResults}
      busy={finderBusy}
      onquery={(text) => void runFind(text)}
      onpick={pickFound}
      onclose={() => (finderOpen = false)}
    />
  {/if}

  {#if paletteOpen}
    <CommandPalette
      commands={palette}
      recent={recentCommands}
      onrun={runCommand}
      onclose={() => (paletteOpen = false)}
    />
  {/if}

  {#if dropMenu}
    <DropMenu
      actions={dropMenu.actions}
      x={dropMenu.x}
      y={dropMenu.y}
      onpick={(action) => void runDropAction(action)}
      onclose={() => (dropMenu = null)}
    />
  {/if}

  {#if rebaseOpen}
    <RebaseEditor
      base={rebaseBase}
      plan={rebasePlan}
      published={splitPublished}
      busy={rebaseBusy}
      onplan={(next) => (rebasePlan = next)}
      onrun={() => void runRebase()}
      onclose={() => (rebaseOpen = false)}
    />
  {/if}

  {#if splitOpen && commit.oid}
    <SplitOffDialog
      oid={commit.oid}
      changed={commit.files.map((file) => file.path)}
      published={splitPublished}
      busy={splitBusy}
      onsplit={(paths, message, first) => void runSplit(paths, message, first)}
      onclose={() => (splitOpen = false)}
    />
  {/if}

  {#if hooks.open}
    <HooksPanel
      overview={hooks.overview}
      editing={hooks.editing}
      body={hooks.body}
      onedit={(name) => {
        const id = repository.current?.repo;
        if (id) void hooks.edit(id, name);
      }}
      onbody={(text) => (hooks.body = text)}
      onsave={() => {
        const id = repository.current?.repo;
        if (id) void hooks.save(id);
      }}
      oncancel={() => (hooks.editing = null)}
      ontoggle={(name, enabled) => {
        const id = repository.current?.repo;
        if (id) void hooks.toggle(id, name, enabled);
      }}
      onadopt={(path) => {
        const id = repository.current?.repo;
        if (id) void hooks.adopt(id, path);
      }}
      onrun={(name) => {
        const id = repository.current?.repo;
        if (id) void hooks.dryRun(id, name);
      }}
      lastRun={hooks.lastRun}
      running={hooks.running}
      onclose={() => hooks.close()}
    />
  {/if}

  {#if settingsOpen}
    <SettingsPanel
      value={settings.current}
      tokenHost={network.tokenHost}
      tokenStored={network.tokenStored}
      onstoretoken={(token) => void network.storeToken(token)}
      onforgettoken={() => void network.forgetToken()}
      onchange={(key, next) => void changeSetting(key, next)}
      onreset={() => void settings.reset()}
      onclose={() => (settingsOpen = false)}
    />
  {/if}

  {#if errors.current}
    <GitErrorDialog error={errors.current} ondismiss={() => errors.dismiss()} />
  {/if}

  <StatusBar
    repository={repo?.name ?? "No repository"}
    branch={repo ? repository.headLabel : undefined}
    upstream={tracked?.upstream ?? undefined}
    ahead={tracked?.ahead ?? 0}
    behind={tracked?.behind ?? 0}
    summary={repo ? `${graph.rows.length} commits · ${repo.branches.length} refs` : "Milestone C"}
    version={info?.version}
    status={network.running
      ? (network.progress ?? `${network.running}…`)
      : repository.error
        ? "Error"
        : repository.busy
          ? "Working…"
          : "Ready"}
    problems={output.problems}
    onproblems={() => output.toggle()}
  />
</div>

<style>
  .app {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--surface-base);
  }

  .workspace {
    display: flex;
    flex: 1 1 auto;
    min-height: 0;
  }

  .left-column,
  .right-area {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }

  .right-area {
    flex: 1 1 0;
  }

  .top-row {
    display: flex;
    min-height: 0;
    min-width: 0;
  }

  .pane {
    display: flex;
    min-width: 0;
    min-height: 0;
  }

  .pane > :global(.panel) {
    flex: 1 1 auto;
  }

  .grow {
    flex: 1 1 0;
  }

  .ref-filter {
    width: 140px;
    height: 18px;
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: 10px;
  }

  .files {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .note {
    margin: 0;
    padding: var(--sp-5);
    font-size: var(--fs-dense);
    color: var(--text-secondary);
  }

  .detail {
    padding: var(--sp-6);
    font-size: var(--fs-dense);
    user-select: text;
  }

  .error {
    margin: 0 0 var(--sp-4);
    color: var(--status-delete);
  }

  /* Raw Git output is never reformatted or truncated (INV-05). */
  .raw {
    margin: 0;
    padding: var(--sp-5);
    background: var(--surface-input);
    border-radius: var(--r-sm);
    font-family: var(--font-mono);
    font-size: var(--fs-code);
    white-space: pre;
    overflow: auto;
  }

  .commit-actions {
    display: flex;
    gap: var(--sp-3);
  }

  .commit-actions button {
    height: 20px;
    padding: 0 var(--sp-4);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }

  .commit-actions button:hover {
    border-color: var(--status-ref);
  }

  .subject {
    margin: 0 0 var(--sp-4);
    font-weight: 600;
  }

  .body {
    margin: 0 0 var(--sp-5);
    font-family: inherit;
    white-space: pre-wrap;
    color: var(--text-secondary);
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--sp-3) var(--sp-6);
    margin: 0 0 var(--sp-5);
  }

  dt {
    color: var(--text-secondary);
  }

  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
</style>
