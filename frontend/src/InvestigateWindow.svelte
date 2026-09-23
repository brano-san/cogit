<script lang="ts">
  import { onMount, untrack } from "svelte";
  import DiffView from "$components/diff/DiffView.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import TooltipLayer from "$components/common/TooltipLayer.svelte";
  import Splitter from "$components/layout/Splitter.svelte";
  import BlamePanel from "$components/investigate/BlamePanel.svelte";
  import DeeperBar from "$components/investigate/DeeperBar.svelte";
  import InvestigateToolbar from "$components/investigate/InvestigateToolbar.svelte";
  import LogPanel from "$components/investigate/LogPanel.svelte";
  import NavigationPanel from "$components/investigate/NavigationPanel.svelte";
  import OriginCandidates from "$components/investigate/OriginCandidates.svelte";
  import OriginView from "$components/investigate/OriginView.svelte";
  import { installChildWindow, onMenuAction } from "$lib/child-window";
  import type { DiffSpec, RepoId } from "$lib/ipc";
  import {
    cancelOriginSearch,
    investigateBlame,
    investigateLog,
    originCandidates,
  } from "$lib/ipc/investigate";
  import { commitOf } from "$lib/investigate/blame";
  import { commandOf, perspectiveOf, type InvestigateCommand } from "$lib/investigate/menu";
  import { investigateTitle, parseInvestigate } from "$lib/investigate/params";
  import { panelsOf } from "$lib/investigate/perspectives";
  import { InvestigateSession, type InvestigateBackend } from "$lib/investigate/session.svelte";
  import { avatars } from "$stores/avatars.svelte";
  import { diff } from "$stores/diff.svelte";
  import { settings } from "$stores/settings.svelte";

  const request = parseInvestigate(window.location.search);

  function backendFor(repo: RepoId): InvestigateBackend {
    return {
      log: (path, rev, follow) => investigateLog(repo, path, rev, follow),
      blame: (path, rev, ignoreWhitespace) => investigateBlame(repo, path, rev, ignoreWhitespace),
      origins: (query, onStarted) => originCandidates(repo, query, onStarted),
      cancel: (id) => cancelOriginSearch(id),
    };
  }

  const session = request ? new InvestigateSession(backendFor(request.repo), request.start) : null;

  let helpOpen = $state(false);
  let navFraction = $state(0.34);
  let now = $state(Math.floor(Date.now() / 1000));

  const panels = $derived(session ? panelsOf(session.perspective) : null);

  // `onMount` does not track: the session reads its own state while starting.
  onMount(() => {
    document.title = request ? investigateTitle(request.start.path, request.repoName) : "Investigate";
    const undoWindow = installChildWindow(window);
    const undoMenu = onMenuAction(window, (action) => {
      const command = commandOf(action);
      if (command) run(command);
    });
    void settings.load().then(() => avatars.apply(settings.current.avatars === "gravatar"));
    void session?.start();
    const tick = setInterval(() => (now = Math.floor(Date.now() / 1000)), 60_000);
    return () => {
      undoWindow();
      undoMenu();
      clearInterval(tick);
      session?.dispose();
    };
  });

  $effect(() => {
    const rows = session?.sections.flatMap((section) => section.rows) ?? [];
    const authors = rows.slice(0, 200).map((row) => ({ authorName: row.author, authorEmail: row.email }));
    untrack(() => void avatars.load(authors));
  });

  /** The Diff perspective reads the shared diff store, as the compare window does. */
  $effect(() => {
    if (!session || !request || session.perspective !== "diff") return;
    const { path, rev } = session.location;
    const spec: DiffSpec =
      rev === null ? { kind: "workTreeVsIndex" } : { kind: "commitVsParent", oid: rev };
    untrack(() => void diff.load(request.repo, spec, path));
  });

  /** The native menu cannot grey items out, so a command that does not apply is ignored. */
  function applies(command: InvestigateCommand): boolean {
    if (!session) return false;
    switch (command) {
      case "back":
        return session.canGoBack;
      case "forward":
        return session.canGoForward;
      case "go-deeper":
        return !!session.candidate?.deeper;
      case "copy-line":
        return session.selectedLine !== null && !!session.blame;
      default:
        return true;
    }
  }

  async function copy(text: string) {
    const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
    await writeText(text);
  }

  function selectedCommitId(): string | null {
    if (!session) return null;
    const line = session.selectedLine === null ? null : session.blame?.lines[session.selectedLine];
    const commit = line && session.blame ? commitOf(session.blame, line) : undefined;
    if (commit && !commit.uncommitted) return commit.oid;
    return session.location.rev;
  }

  /** In the Diff perspective the change keys belong to the diff, which listens for them. */
  function forwardToDiff(shiftKey: boolean) {
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "F6", shiftKey }));
  }

  function run(command: InvestigateCommand) {
    if (!session || !applies(command)) return;
    const perspective = perspectiveOf(command);
    if (perspective) {
      session.setPerspective(perspective);
      return;
    }
    switch (command) {
      case "copy-line": {
        const line = session.selectedLine === null ? null : session.blame?.lines[session.selectedLine];
        if (line) void copy(line.text);
        break;
      }
      case "copy-commit-id": {
        const oid = selectedCommitId();
        if (oid) void copy(oid);
        break;
      }
      case "copy-path":
        void copy(session.location.path);
        break;
      case "follow-renames":
        void session.setFollow(!session.follow);
        break;
      case "ignore-whitespace":
        void session.setIgnoreWhitespace(!session.ignoreWhitespace);
        break;
      case "refresh":
        void session.refresh();
        break;
      case "back":
        void session.back();
        break;
      case "forward":
        void session.forward();
        break;
      case "go-deeper":
        void session.goDeeper();
        break;
      case "close-card":
        session.closeCard();
        break;
      case "previous-change":
      case "next-change":
        if (session.perspective === "diff") forwardToDiff(command === "previous-change");
        else session.moveToChange(command === "next-change" ? 1 : -1);
        break;
      case "newer-version":
        void session.step(-1);
        break;
      case "older-version":
        void session.step(1);
        break;
      case "help":
        helpOpen = true;
        break;
    }
  }

  function resize(delta: number) {
    navFraction = Math.min(0.8, Math.max(0.12, navFraction + delta));
  }
</script>

<TooltipLayer />

<div class="window">
  {#if !session || !request || !panels}
    <p class="note">
      This window needs a file to investigate. Open it from the Diff panel or a file's context
      menu rather than by hand.
    </p>
  {:else}
    <InvestigateToolbar {session} />
    <div class="body">
      <div class="navigation" style:flex-basis="{navFraction * 100}%">
        <NavigationPanel {session} />
      </div>
      <Splitter
        direction="horizontal"
        value={navFraction}
        label="Resize Navigation"
        onchange={resize}
        onreset={() => (navFraction = 0.34)}
      />
      <div class="lower">
        {#if panels.log}
          <LogPanel repo={request.repo} {session} />
        {:else if panels.diff}
          {#if diff.error}
            <p class="note error">{diff.error.message}</p>
          {:else if diff.diff && diff.path}
            <div class="diff">
              <DiffView
                diff={diff.diff}
                path={diff.path}
                stageable={false}
                whitespace={diff.whitespace}
                onwhitespace={(mode) => void diff.setWhitespace(request.repo, mode)}
              />
            </div>
          {:else}
            <p class="note">Loading…</p>
          {/if}
        {:else}
          {#if panels.blame}<BlamePanel {session} {now} />{/if}
          {#if panels.candidates}<OriginCandidates {session} />{/if}
          {#if panels.origin}<OriginView {session} />{/if}
          <DeeperBar {session} />
        {/if}
      </div>
    </div>
  {/if}
</div>

{#if helpOpen}
  <Dialog title="How Investigate Works" onclose={() => (helpOpen = false)} width="min(560px, 92vw)">
    <div class="help">
      <p>
        Investigate traces where lines came from. <b>Navigation</b> lists the commits that changed
        the file, following renames; files reached later get their own header.
      </p>
      <p>
        Picking a line in <b>Blame</b> searches, in the background, for the origin of its block:
        in place, elsewhere in the file, or in other files, allowing for edits made on the way.
        The best candidate is chosen; <b>Blame+Origins</b> and <b>Origins</b> show the rest.
      </p>
      <p>
        <b>Go Deeper</b> (Ctrl+D) opens the version before the origin at the matching line and
        searches again. Back and Forward (Alt+Left, Alt+Right) walk the steps taken.
      </p>
      <p>
        Line markers: <span class="mono">+</span> added, <span class="mono">~</span> changed in place,
        <span class="mono">M</span> written by a merge commit.
      </p>
    </div>
  </Dialog>
{/if}

<style>
  .window {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--surface-base);
    color: var(--text-primary);
    font-family: var(--font-ui);
  }

  .body {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    min-height: 0;
  }

  .navigation {
    display: flex;
    flex: 0 0 auto;
    flex-direction: column;
    min-height: 80px;
  }

  .lower {
    display: flex;
    flex: 1 1 0;
    min-height: 0;
  }

  .diff {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    min-width: 0;
  }

  .note {
    margin: 0;
    padding: var(--sp-6);
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .error {
    color: var(--status-delete);
  }

  .help p {
    margin: 0 0 var(--sp-4);
    line-height: var(--lh-ui);
  }
</style>
