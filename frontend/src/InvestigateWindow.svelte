<script lang="ts">
  import DiffView from "$components/diff/DiffView.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import TooltipLayer from "$components/common/TooltipLayer.svelte";
  import Splitter from "$components/layout/Splitter.svelte";
  import BlamePanel from "$components/investigate/BlamePanel.svelte";
  import DeeperBar from "$components/investigate/DeeperBar.svelte";
  import InvestigateToolbar from "$components/investigate/InvestigateToolbar.svelte";
  import LogPanel from "$components/investigate/LogPanel.svelte";
  import MenuBar from "$components/investigate/MenuBar.svelte";
  import NavigationPanel from "$components/investigate/NavigationPanel.svelte";
  import OriginCandidates from "$components/investigate/OriginCandidates.svelte";
  import OriginView from "$components/investigate/OriginView.svelte";
  import { closesWindow } from "$lib/child-window";
  import { closeThisWindow, type RepoId } from "$lib/ipc";
  import {
    cancelOriginSearch,
    investigateBlame,
    investigateLog,
    originCandidates,
  } from "$lib/ipc/investigate";
  import { commitOf } from "$lib/investigate/blame";
  import { MENUS, commandForKey, type InvestigateCommand } from "$lib/investigate/menu";
  import { investigateTitle, parseInvestigate } from "$lib/investigate/params";
  import { panelsOf, type Perspective } from "$lib/investigate/perspectives";
  import { InvestigateSession, type InvestigateBackend } from "$lib/investigate/session.svelte";
  import { suppressNativeMenu } from "$lib/native-menu";
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

  let menuOpen = $state<string | null>(null);
  let helpOpen = $state(false);
  let navFraction = $state(0.34);
  let now = $state(Math.floor(Date.now() / 1000));

  const panels = $derived(session ? panelsOf(session.perspective) : null);

  // Nothing in a Git client is a web page (R-127).
  $effect(() => suppressNativeMenu(document));

  $effect(() => {
    void settings.load().then(() => avatars.apply(settings.current.avatars === "gravatar"));
    void session?.start();
    const tick = setInterval(() => (now = Math.floor(Date.now() / 1000)), 60_000);
    return () => {
      clearInterval(tick);
      session?.dispose();
    };
  });

  $effect(() => {
    document.title = session
      ? investigateTitle(session.location.path, request?.repoName ?? "")
      : "Investigate";
  });

  $effect(() => {
    const rows = session?.sections.flatMap((section) => section.rows) ?? [];
    void avatars.load(rows.slice(0, 200).map((row) => ({ authorName: row.author, authorEmail: row.email })));
  });

  /** The Diff perspective reads the shared diff store, as the compare window does. */
  $effect(() => {
    if (!session || !request || session.perspective !== "diff") return;
    const { path, rev } = session.location;
    void diff.load(
      request.repo,
      rev === null ? { kind: "workTreeVsIndex" } : { kind: "commitVsParent", oid: rev },
      path,
    );
  });

  function stateOf(id: InvestigateCommand): { enabled: boolean; checked: boolean } {
    if (!session) return { enabled: id === "close", checked: false };
    const blame = panels?.blame ?? false;
    if (id.startsWith("perspective:")) {
      return { enabled: true, checked: session.perspective === id.slice("perspective:".length) };
    }
    switch (id) {
      case "back":
        return { enabled: session.canGoBack, checked: false };
      case "forward":
        return { enabled: session.canGoForward, checked: false };
      case "goDeeper":
        return { enabled: !!session.candidate?.deeper, checked: false };
      case "closeCard":
        return { enabled: session.cardOpen, checked: false };
      case "copyLine":
        return { enabled: session.selectedLine !== null && !!session.blame, checked: false };
      case "previousChange":
      case "nextChange":
        return { enabled: blame && session.changes.length > 0, checked: false };
      case "followRenames":
        return { enabled: true, checked: session.follow };
      case "ignoreWhitespace":
        return { enabled: true, checked: session.ignoreWhitespace };
      default:
        return { enabled: true, checked: false };
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

  function run(id: InvestigateCommand) {
    if (!session) {
      if (id === "close") void closeThisWindow();
      return;
    }
    if (!stateOf(id).enabled) return;
    if (id.startsWith("perspective:")) {
      session.setPerspective(id.slice("perspective:".length) as Perspective);
      return;
    }
    switch (id) {
      case "close":
        void closeThisWindow();
        break;
      case "copyLine": {
        const line = session.selectedLine === null ? null : session.blame?.lines[session.selectedLine];
        if (line) void copy(line.text);
        break;
      }
      case "copyCommitId": {
        const oid = selectedCommitId();
        if (oid) void copy(oid);
        break;
      }
      case "copyPath":
        void copy(session.location.path);
        break;
      case "followRenames":
        void session.setFollow(!session.follow);
        break;
      case "ignoreWhitespace":
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
      case "goDeeper":
        void session.goDeeper();
        break;
      case "closeCard":
        session.closeCard();
        break;
      case "previousChange":
        session.moveToChange(-1);
        break;
      case "nextChange":
        session.moveToChange(1);
        break;
      case "newerVersion":
        void session.step(-1);
        break;
      case "olderVersion":
        void session.step(1);
        break;
      case "help":
        helpOpen = true;
        break;
    }
  }

  /** Capture phase: the window's own keys come before a view's, and Esc always closes
      the window unless a menu or the help is open. */
  function onkeydown(event: KeyboardEvent) {
    if (helpOpen) return;
    if (menuOpen !== null && event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      menuOpen = null;
      return;
    }
    if (closesWindow(event)) {
      event.preventDefault();
      void closeThisWindow();
      return;
    }
    const command = commandForKey(event);
    if (!command) return;
    if (session?.perspective === "diff" && (command === "nextChange" || command === "previousChange")) return;
    event.preventDefault();
    run(command);
  }

  function resize(delta: number) {
    navFraction = Math.min(0.8, Math.max(0.12, navFraction + delta));
  }
</script>

<svelte:window onkeydowncapture={onkeydown} />

<TooltipLayer />

<div class="window">
  <MenuBar menus={MENUS} bind:open={menuOpen} {stateOf} oncommand={run} />

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
