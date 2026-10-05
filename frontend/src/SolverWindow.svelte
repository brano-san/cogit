<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { untrack } from "svelte";
  import ConfirmDialog from "$components/common/ConfirmDialog.svelte";
  import TooltipLayer from "$components/common/TooltipLayer.svelte";
  import SolverBand from "$components/solver/SolverBand.svelte";
  import ResolveCloseDialog from "$components/solver/ResolveCloseDialog.svelte";
  import SolverToolbar from "$components/solver/SolverToolbar.svelte";
  import SolverWholeFile from "$components/solver/SolverWholeFile.svelte";
  import { visibleCenter } from "$lib/diff-band";
  import {
    cancelMergeTool,
    closeThisWindow,
    launchMergeTool,
    markConflictResolved,
    mergeResolved,
    mergeToolsRunning,
    onMergeToolFinished,
    popupContextMenu,
    resolveConflict,
    resolveConflictText,
    saveConflictText,
    solverData,
    type MergeToolFinished,
    type SolverData,
  } from "$lib/ipc";
  import { closeGuard, installChildWindow, onMenuAction } from "$lib/child-window";
  import { failureText } from "$lib/merge-params";
  import { modals } from "$lib/modal-stack";
  import { ON_MAC, primary } from "$lib/platform";
  import { followSettings } from "$lib/settings-sync";
  import { SolverEditors, languageExtension, type PaneName, type Snapshot } from "$lib/solver-editor";
  import {
    resolveClose,
    saveFlow,
    toolFollowUp,
    toolPrompt,
    type ResolveChoice,
    type ResolveClose,
  } from "$lib/solver-flow";
  import { bandConnectors, panesOf, type SolverLayout } from "$lib/solver-geometry";
  import { solverKey } from "$lib/solver-keys";
  import { solverMenu } from "$lib/solver-menu";
  import {
    composeSave,
    conflictsLeftLabel,
    docsForDeleted,
    docsFromRegions,
    nextHunk,
    paneTone,
    textToLines,
    type Hunk,
    type TakeAction,
  } from "$lib/solver-model";
  import { parseSolver } from "$lib/solver-params";
  import { loadPrefs, savePrefs, type SolverPrefs } from "$lib/solver-prefs";
  import { confirmation } from "$stores/confirm.svelte";
  import { settings } from "$stores/settings.svelte";

  const request = parseSolver(window.location.search);
  const win = getCurrentWindow();

  // No browser menu, and Esc / Ctrl+W close the window.
  $effect(() => installChildWindow(window));
  // What Preferences changes in the main window reaches this one too (F-335).
  $effect(() => followSettings(() => void settings.reload()));

  let data = $state.raw<SolverData | null>(null);
  let loadFailed = $state<string | null>(null);
  let prefs = $state<SolverPrefs>(loadPrefs());
  let editors = $state.raw<SolverEditors | null>(null);
  let snapshot = $state.raw<Snapshot | null>(null);
  /** Bumped by every scroll and change: the bands and the buttons are drawn from the panes. */
  let tick = $state(0);
  let oursEl = $state<HTMLDivElement>();
  let resultEl = $state<HTMLDivElement>();
  let theirsEl = $state<HTMLDivElement>();
  let saveFailed = $state<string | null>(null);
  let note = $state<string | null>(null);
  let toolRunning = $state(false);
  let saving = $state(false);
  /** The close question on screen, and how it is answered. */
  let closing = $state.raw<ResolveClose | null>(null);
  let closeAnswer: ((choice: ResolveChoice) => void) | null = null;
  /** Written to the index: the window closes behind its own Save without asking. */
  let saved = false;

  const docs = $derived.by(() => {
    if (!data || data.binary || data.tooLarge) return null;
    if (data.missingOurs || data.missingTheirs || data.regions.length === 0) {
      return docsForDeleted({ ours: data.ours, theirs: data.theirs });
    }
    return docsFromRegions(data.regions);
  });
  /** Line by line: there are hunks, and so ribbons, buttons and a counter. */
  const merged = $derived(
    data !== null && docs !== null && !data.missingOurs && !data.missingTheirs && data.regions.length > 0,
  );
  const deleted = $derived(
    data && docs && (data.missingOurs || data.missingTheirs)
      ? { ours: data.missingOurs, theirs: data.missingTheirs }
      : null,
  );
  const panes = $derived(panesOf(deleted ? "all" : prefs.layout));
  const hunks = $derived<readonly Hunk[]>(docs?.hunks ?? []);
  const hunkIds = $derived(hunks.map((hunk) => hunk.id));
  const indexOf = $derived(new Map(hunkIds.map((id, at) => [id, at])));
  const unresolved = $derived(snapshot?.unresolved ?? []);
  const open = $derived(new Set(unresolved));
  const current = $derived(snapshot?.currentId ?? null);
  const aligned = $derived(settings.current.diffLayout === "aligned");

  async function load() {
    if (!request) return;
    try {
      data = await solverData(request.repo, request.path);
      loadFailed = null;
    } catch (err) {
      loadFailed = failureText(err);
    }
  }

  $effect(() => {
    void settings.load();
    if (!request) return;
    void load().then(async () => {
      if ((await mergeToolsRunning(request.repo)).includes(request.path)) toolRunning = true;
      else if (request.tool && data) void runExternal();
    });
  });

  $effect(() => {
    document.title = request ? `Cogit — Conflict Solver — ${request.path}` : "Cogit — Conflict Solver";
  });

  // The editors are made once the page has somewhere to put them, and again for another read.
  $effect(() => {
    const made = docs;
    if (!made || !oursEl || !resultEl || !theirsEl || !request) return;
    const created = new SolverEditors(
      { ours: oursEl, result: resultEl, theirs: theirsEl },
      made,
      editorOptions(),
      () => {
        snapshot = created.snapshot;
        tick += 1;
      },
    );
    editors = created;
    snapshot = created.snapshot;
    void languageExtension(request.path).then((extension) => created.setLanguage(extension));
    return () => {
      created.destroy();
      editors = null;
      snapshot = null;
    };
  });

  function editorOptions() {
    return {
      aligned: settings.current.diffLayout === "aligned",
      baseChanges: prefs.baseChanges,
      shown: { ours: panes.ours, theirs: panes.theirs },
      resultBelow: panes.below,
    };
  }

  $effect(() => {
    const options = {
      aligned,
      baseChanges: prefs.baseChanges,
      shown: { ours: panes.ours, theirs: panes.theirs },
      resultBelow: panes.below,
    };
    const made = editors;
    // The editors report back by writing `tick`, which this effect must not then depend on.
    untrack(() => made?.setOptions(options));
  });

  function setLayout(layout: SolverLayout) {
    prefs = { ...prefs, layout };
    savePrefs(prefs);
  }

  function setBaseChanges(baseChanges: boolean) {
    prefs = { ...prefs, baseChanges };
    savePrefs(prefs);
  }

  const geometry = $derived.by(() => {
    void tick;
    void panes;
    return editors?.geometry() ?? null;
  });

  function ribbons(left: "ours" | "result", right: "result" | "theirs") {
    const geo = geometry;
    if (!geo) return [];
    return bandConnectors(
      geo[left],
      geo[right],
      geo.scroll[left],
      geo.scroll[right],
      Math.max(geo.viewport[left], geo.viewport[right]),
      hunkIds,
      9,
    );
  }

  const oursBand = $derived(ribbons("ours", "result"));
  const theirsBand = $derived(ribbons("result", "theirs"));
  const topBand = $derived(ribbons("ours", "theirs" as "result"));

  const hunkOf = $derived(new Map(hunks.map((hunk) => [hunk.id, hunk])));
  const tone = (side: "ours" | "theirs") => (id: number) => {
    const hunk = hunkOf.get(id);
    return hunk ? paneTone(hunk, side, prefs.baseChanges) : null;
  };
  const shows = (side: "ours" | "theirs") => (id: number) => {
    const at = indexOf.get(id);
    return at === undefined ? false : (snapshot?.actions[at]?.[side] ?? false);
  };

  /** Result below: the Result has no band beside it, so its buttons sit in its own margin. */
  const belowButtons = $derived.by(() => {
    const geo = geometry;
    if (!geo || !panes.below || !merged) return [];
    return hunkIds.flatMap((id, at) => {
      const edge = geo.result[at];
      if (!edge) return [];
      const y = visibleCenter(edge.top - geo.scroll.result, edge.bottom - geo.scroll.result, geo.viewport.result, 9);
      return y === null ? [] : [{ id, y }];
    });
  });

  function titleOf(pane: PaneName): string {
    if (!data) return "";
    if (pane === "ours") return `${data.context.ours} ("ours")`;
    if (pane === "theirs") return `${data.context.theirs} ("theirs")`;
    return "Working Tree";
  }

  function step(kind: "change" | "conflict", by: 1 | -1) {
    if (!editors || !snapshot) return;
    const to = nextHunk(kind === "change" ? hunkIds : snapshot.unresolved, snapshot.currentId, by);
    if (to !== null) editors.goTo(to);
  }

  /** The current hunk, or the first undecided conflict when none is. */
  function target(): number | null {
    return snapshot?.currentId ?? snapshot?.unresolved[0] ?? null;
  }

  function take(action: TakeAction, advance = true) {
    const id = target();
    const made = editors;
    if (id === null || !made || !merged) return;
    made.take(id, action);
    if (!advance) return;
    queueMicrotask(() => {
      const next = nextHunk(made.snapshot.unresolved, id, 1);
      if (next !== null && next !== id) made.goTo(next);
    });
  }

  function takeAt(id: number, action: TakeAction) {
    editors?.setCurrent(id);
    editors?.take(id, action);
  }

  function labels() {
    return { ours: data?.context.ours ?? "ours", theirs: data?.context.theirs ?? "theirs" };
  }

  async function announceAndClose() {
    if (!request) return;
    await mergeResolved(request.repo, request.path);
    saved = true;
    await closeThisWindow();
  }

  /** The Result as the file should hold it: undecided hunks as conflict markers. */
  function textToWrite(): string {
    if (!editors || !docs) return "";
    const left = snapshot?.unresolved.length ?? 0;
    return left > 0
      ? composeSave(textToLines(editors.resultText), editors.spans(), docs.hunks, labels(), editors.decided())
      : editors.resultText;
  }

  /** Ctrl+S: the working file gets the Result and the window stays; nothing is staged, so
      the file is still conflicted until Mark Resolved, which is offered right after (as in
      SmartGit) unless the window is closing. True when it was written. */
  async function save(offerResolve = true): Promise<boolean> {
    if (!editors || !request || !docs || saving || toolRunning) return false;
    const { repo, path } = request;
    saving = true;
    saveFailed = null;
    try {
      await saveConflictText(repo, path, textToWrite(), data?.stages ?? null);
      editors.markSaved();
      note = "Saved. The file stays conflicted until you Mark Resolved.";
      if (offerResolve && (snapshot?.unresolved.length ?? 0) === 0) void offerMarkResolved(path);
      return true;
    } catch (err) {
      saveFailed = err instanceof Error ? err.message : String(err);
      return false;
    } finally {
      saving = false;
    }
  }

  async function offerMarkResolved(path: string) {
    const yes = await confirmation.ask({
      title: "Mark Resolved",
      message: `${path} is saved with every conflict decided. Mark it resolved and stage it now?`,
      confirm: "Mark Resolved",
    });
    if (yes) await markResolved();
  }

  /** `warned`: the close question already said markers stay; staging asks nothing more. */
  async function markResolved(warned = false) {
    if (!editors || !request || !docs || saving || toolRunning) return;
    const { repo, path } = request;
    const stages = data?.stages;
    saving = true;
    saveFailed = null;
    const left = snapshot?.unresolved.length ?? 0;
    const text =
      left > 0
        ? composeSave(textToLines(editors.resultText), editors.spans(), docs.hunks, labels(), editors.decided())
        : editors.resultText;
    const result = await saveFlow({
      unresolved: warned ? 0 : left,
      confirmMarkers: () =>
        confirmation.ask({
          title: "Save with Conflict Markers",
          message: `${left === 1 ? "1 conflict is" : `${left} conflicts are`} not resolved. ${path} will be saved with conflict markers in place of ${left === 1 ? "it" : "them"} and staged. Save anyway?`,
          confirm: "Save",
          warning: true,
        }),
      write: () => resolveConflictText(repo, path, text, stages),
      announce: () => mergeResolved(repo, path),
      saved: () => (saved = true),
      close: () => closeThisWindow(),
    });
    saving = false;
    if (result.kind === "failed") saveFailed = result.message;
  }

  /** A whole side, or the deletion, for the file: git checks it out and stages it. */
  async function resolveWhole(side: "ours" | "theirs") {
    if (!request) return;
    saveFailed = null;
    try {
      await resolveConflict(request.repo, request.path, side, data?.stages);
      await announceAndClose();
    } catch (err) {
      saveFailed = failureText(err);
    }
  }

  async function deleteFile() {
    if (!request || !data) return;
    const yes = await confirmation.ask({
      title: "Delete File",
      message: `Resolve ${request.path} by deleting it? The deletion is staged. Undo can bring the file back.`,
      confirm: "Delete File",
      warning: true,
    });
    if (!yes) return;
    // The side that lost the file is the one that is missing: taking it is the deletion.
    await resolveWhole(data.missingOurs ? "ours" : "theirs");
  }

  async function runExternal() {
    if (!request || toolRunning || !data) return;
    if (snapshot?.dirty) {
      const go = await confirmation.ask({
        title: "Open in External Tool",
        message:
          "The merge tool works on the file in the working tree, not on what was edited in this window. The edits made here will be lost.",
        confirm: "Open Tool",
        warning: true,
      });
      if (!go) return;
    }
    note = null;
    saveFailed = null;
    // Before the call: a tool that ends at once must find the window waiting for it.
    toolRunning = true;
    try {
      await launchMergeTool(
        request.repo,
        request.path,
        settings.current.mergeExternalTool,
        settings.current.mergeExternalToolArgs,
      );
    } catch (err) {
      toolRunning = false;
      saveFailed = failureText(err);
    }
  }

  async function toolFinished(event: MergeToolFinished) {
    if (!request || event.repo !== request.repo || event.path !== request.path) return;
    toolRunning = false;
    const next = toolFollowUp(event.outcome);
    if (next === "alreadyResolved") {
      await announceAndClose();
      return;
    }
    if (next === "stillConflicted") {
      note = "The merge tool closed, but conflict markers are still in the file. It stays conflicted.";
    } else if (next === "offerResolve") {
      const yes = await confirmation.ask({
        title: "Mark Resolved",
        message: toolPrompt(request.path, event.outcome),
        confirm: "Mark Resolved",
      });
      if (yes) {
        try {
          await markConflictResolved(request.repo, request.path);
          await announceAndClose();
          return;
        } catch (err) {
          saveFailed = failureText(err);
        }
      }
    }
    await load();
  }

  $effect(() => {
    const pending = onMergeToolFinished((event) => void toolFinished(event));
    return () => void pending.then((stop) => stop()).catch(() => {});
  });

  async function cancelTool() {
    if (request) await cancelMergeTool(request.repo, request.path);
  }

  async function askClose(): Promise<boolean> {
    if (toolRunning) {
      const go = await confirmation.ask({
        title: "Merge Tool Running",
        message: "A merge tool still has this file open. Closing the window ends it.",
        confirm: "End Tool and Close",
        warning: true,
      });
      if (go) await cancelTool();
      return go;
    }
    // Undecided conflicts are written as markers; markers typed into the Result count too.
    const undecided = snapshot?.unresolved.length ?? 0;
    const typed = /^(<{7}|={7}|>{7})( |$)/m.test(editors?.resultText ?? "") ? 1 : 0;
    const question = resolveClose(snapshot?.dirty ?? false, undecided || typed);
    const choice = await new Promise<ResolveChoice>((resolve) => {
      closeAnswer = resolve;
      closing = question;
    });
    closing = null;
    closeAnswer = null;
    switch (choice) {
      case "cancel":
        return false;
      case "discard":
        return true;
      case "keep":
        if (!(snapshot?.dirty ?? false)) return true;
        void save(false).then((written) => written && closeThisWindow());
        return false;
      case "resolve":
        // The markers were warned about in the same question: no second one before staging.
        void markResolved(true);
        return false;
    }
  }

  // Esc, Ctrl+W and the ✕ all come here as one close request.
  $effect(() => {
    // Still conflicted until Mark Resolved: closing asks whether to resolve it (and about
    // unsaved edits in the same question).
    const guard = closeGuard(() => !saved && (docs !== null || toolRunning), askClose);
    const pending = win.onCloseRequested(guard);
    return () => void pending.then((stop) => stop()).catch(() => {});
  });

  $effect(() =>
    onMenuAction(window, (action) => {
      if (modals.any) return;
      const takes: Record<string, TakeAction> = {
        "solver-take-ours": "ours",
        "solver-take-theirs": "theirs",
        "solver-take-ours-theirs": "oursTheirs",
        "solver-take-theirs-ours": "theirsOurs",
        "solver-take-base": "base",
      };
      const taken = takes[action];
      if (taken) take(taken, false);
      else if (action === "solver-prev-change") step("change", -1);
      else if (action === "solver-next-change") step("change", 1);
      else if (action === "solver-prev-conflict") step("conflict", -1);
      else if (action === "solver-next-conflict") step("conflict", 1);
      else if (action === "external-tool" || action === "solver-external-tool") void runExternal();
    }),
  );

  async function oncontext(event: MouseEvent, pane: PaneName) {
    event.preventDefault();
    if (!editors || modals.any || toolRunning) return;
    const id = editors.hunkAt(pane, event.clientX, event.clientY);
    if (id !== null) editors.setCurrent(id);
    const menu = solverMenu(win.label, {
      hunk: merged && (id ?? snapshot?.currentId ?? null) !== null,
      changes: merged && hunkIds.length > 0,
      conflicts: merged && unresolved.length > 0,
      tool: true,
    });
    await popupContextMenu(menu, event.clientX, event.clientY).catch(() => {});
  }

  function onkeydown(event: KeyboardEvent) {
    if (modals.any || toolRunning || !docs) return;
    const action = solverKey({
      key: event.key,
      code: event.code,
      ctrl: primary(event, ON_MAC),
      shift: event.shiftKey,
      alt: event.altKey,
    });
    if (action === null) return;
    event.preventDefault();
    if (action === "save") void save();
    else if ("step" in action) step(action.step, action.by);
    else take(action.take);
  }

  // Nothing is worth looking at from a note once the next thing is done.
  $effect(() => {
    void snapshot?.version;
    if (snapshot?.dirty) note = null;
  });
</script>

<svelte:window {onkeydown} />

<TooltipLayer />

<div class="window">
  {#if !request}
    <p class="message">This window needs a conflicted file. Open it from the Files panel.</p>
  {:else if loadFailed && !data}
    <p class="message error">{loadFailed}</p>
  {:else if !data}
    <p class="message">Reading the three sides…</p>
  {:else}
    {#if data.binary || data.tooLarge}
      <SolverWholeFile
        {data}
        path={request.path}
        locked={toolRunning}
        onkeep={(side) => void resolveWhole(side)}
        ondelete={() => void deleteFile()}
        onexternal={() => void runExternal()}
      />
    {:else}
      <SolverToolbar
        layout={prefs.layout}
        onlayout={setLayout}
        baseChanges={prefs.baseChanges}
        onbase={setBaseChanges}
        {merged}
        hasChanges={hunkIds.length > 0}
        hasConflicts={unresolved.length > 0}
        onstep={step}
        conflictsLabel={conflictsLeftLabel(unresolved.length)}
        canTake={target() !== null}
        ontake={(action) => take(action)}
        {deleted}
        onkeep={(side) => void resolveWhole(side)}
        ondelete={() => void deleteFile()}
        onexternal={() => void runExternal()}
        onsave={() => void save()}
        onresolve={() => void markResolved()}
        dirty={snapshot?.dirty ?? false}
        locked={toolRunning}
        {saving}
      />
    {/if}

    {#if toolRunning}
      <div class="banner" role="status">
        <span>Waiting for external tool…</span>
        <button type="button" class="btn sm" onclick={() => void cancelTool()}>Cancel</button>
      </div>
    {/if}
    {#if saveFailed}
      <div class="banner failure" role="alert">
        <pre>{saveFailed}</pre>
        {#if saveFailed.includes("changed since")}
          <button type="button" class="btn sm" onclick={() => void load()}>Reload</button>
        {/if}
      </div>
    {/if}
    {#if note}
      <div class="banner" role="status">{note}</div>
    {/if}

    {#if docs}
      <div class="panes layout-{deleted ? 'all' : prefs.layout}" inert={toolRunning}>
        <section class="pane ours" aria-label="Ours">
          <header title={titleOf("ours")}><span class="truncate">{titleOf("ours")}</span></header>
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="host" bind:this={oursEl} oncontextmenu={(e) => void oncontext(e, "ours")}>
            {#if data.missingOurs}<div class="plate"><span>Deleted in ours</span></div>{/if}
          </div>
        </section>

        <div class="slot band-left">
          <SolverBand
            connectors={oursBand}
            tone={tone("ours")}
            {current}
            {open}
            button="ours"
            show={shows("ours")}
            onact={(id) => takeAt(id, "ours")}
            label="Take Ours"
          />
        </div>

        <section class="pane result" aria-label="Result">
          <header title="Working Tree"
            ><span class="truncate">{titleOf("result")}</span>{#if snapshot?.dirty}<span class="dirty" title="Unsaved edits">&nbsp;●</span>{/if}</header
          >
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="host" bind:this={resultEl} oncontextmenu={(e) => void oncontext(e, "result")}>
            {#each belowButtons as button (button.id)}
              <span class="below" style:top="{button.y}px">
                {#if shows("ours")(button.id)}
                  <button type="button" title="Take Ours" aria-label="Take Ours" onclick={() => takeAt(button.id, "ours")}>»</button>
                {/if}
                {#if shows("theirs")(button.id)}
                  <button type="button" title="Take Theirs" aria-label="Take Theirs" onclick={() => takeAt(button.id, "theirs")}>«</button>
                {/if}
              </span>
            {/each}
          </div>
        </section>

        <div class="slot band-right">
          <SolverBand
            connectors={theirsBand}
            tone={tone("theirs")}
            {current}
            {open}
            button="theirs"
            show={shows("theirs")}
            onact={(id) => takeAt(id, "theirs")}
            label="Take Theirs"
          />
        </div>

        <section class="pane theirs" aria-label="Theirs">
          <header title={titleOf("theirs")}><span class="truncate">{titleOf("theirs")}</span></header>
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="host" bind:this={theirsEl} oncontextmenu={(e) => void oncontext(e, "theirs")}>
            {#if data.missingTheirs}<div class="plate"><span>Deleted in theirs</span></div>{/if}
          </div>
        </section>

        <div class="slot band-top">
          <SolverBand
            connectors={topBand}
            tone={tone("ours")}
            {current}
            {open}
            button={null}
            show={() => false}
            onact={() => {}}
            label=""
          />
        </div>
      </div>
      <footer>
        <span>{data.crlf ? "CRLF" : "LF"}</span>
        {#if merged}<span>{conflictsLeftLabel(unresolved.length)}</span>{/if}
      </footer>
    {/if}
  {/if}
</div>

{#if closing}
  <ResolveCloseDialog path={request?.path ?? ""} question={closing} onanswer={(choice) => closeAnswer?.(choice)} />
{/if}

{#if confirmation.open}
  <ConfirmDialog
    title={confirmation.open.title}
    message={confirmation.open.message}
    confirm={confirmation.open.confirm}
    warning={confirmation.open.warning}
    onanswer={(yes) => confirmation.answer(yes)}
  />
{/if}

<style>
  .window {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--surface-base);
    color: var(--text-primary);
  }

  .message {
    margin: 0;
    padding: var(--sp-6);
    font-size: var(--fs-dense);
    color: var(--text-secondary);
  }

  .error {
    color: var(--status-danger);
    user-select: text;
    white-space: pre-wrap;
  }

  .dirty {
    color: var(--status-modify);
  }

  .banner {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    flex: 0 0 auto;
    padding: var(--sp-3) var(--sp-5);
    font-size: var(--fs-dense);
    background: var(--badge-warning-bg);
    color: var(--badge-warning-fg);
    border-bottom: 1px solid var(--border);
  }

  .banner.failure {
    background: var(--bg-elevated);
    color: var(--status-danger);
  }

  .banner pre {
    max-height: 10em;
    margin: 0;
    overflow: auto;
    font-family: var(--font-mono);
    white-space: pre-wrap;
    user-select: text;
  }

  .panes {
    display: grid;
    flex: 1 1 auto;
    min-height: 0;
    background: var(--bg-editor);
  }

  .panes[inert] {
    opacity: 0.6;
  }

  .layout-all {
    grid-template-columns: minmax(0, 1fr) 42px minmax(0, 1fr) 42px minmax(0, 1fr);
    grid-template-areas: "ours bandl result bandr theirs";
  }

  .layout-oursResult {
    grid-template-columns: minmax(0, 1fr) 42px minmax(0, 1fr);
    grid-template-areas: "ours bandl result";
  }

  .layout-resultTheirs {
    grid-template-columns: minmax(0, 1fr) 42px minmax(0, 1fr);
    grid-template-areas: "result bandr theirs";
  }

  .layout-resultBelow {
    grid-template-columns: minmax(0, 1fr) 42px minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr) minmax(0, 1fr);
    grid-template-areas:
      "ours bandt theirs"
      "result result result";
  }

  .ours {
    grid-area: ours;
  }

  .result {
    grid-area: result;
  }

  .theirs {
    grid-area: theirs;
  }

  .band-left {
    grid-area: bandl;
  }

  .band-right {
    grid-area: bandr;
  }

  .band-top {
    grid-area: bandt;
  }

  .slot {
    min-height: 0;
  }

  .layout-all .band-top,
  .layout-oursResult .band-top,
  .layout-resultTheirs .band-top,
  .layout-resultBelow .band-left,
  .layout-resultBelow .band-right {
    display: none;
  }

  .layout-oursResult .theirs,
  .layout-oursResult .band-right,
  .layout-resultTheirs .ours,
  .layout-resultTheirs .band-left {
    display: none;
  }

  .pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    border-bottom: 1px solid var(--border);
  }

  header {
    display: flex;
    flex: 0 0 auto;
    align-items: center;
    height: 22px;
    padding: 0 var(--sp-4);
    background: var(--bg-panel);
    border-bottom: 1px solid var(--border);
    color: var(--fg-secondary);
    font-size: var(--fs-header);
    font-weight: 600;
    user-select: none;
  }

  .truncate {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .host {
    position: relative;
    flex: 1 1 auto;
    min-height: 0;
  }

  .plate {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    z-index: 2;
    pointer-events: none;
  }

  .plate span {
    padding: var(--sp-2) var(--sp-6);
    background: var(--badge-warning-bg);
    color: var(--badge-warning-fg);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
  }

  .below {
    position: absolute;
    right: 22px;
    z-index: 3;
    display: flex;
    gap: 2px;
    transform: translateY(-50%);
  }

  .below button {
    width: 18px;
    height: 18px;
    padding: 0;
    border: 0;
    background: var(--diff-center-gutter-bg);
    color: var(--diff-center-gutter-action);
    border-radius: var(--r-sm);
    font-size: 13px;
    font-weight: 700;
    line-height: 18px;
    cursor: pointer;
  }

  .below button:hover {
    color: var(--diff-center-gutter-action-hover);
  }

  footer {
    display: flex;
    gap: var(--sp-5);
    flex: 0 0 auto;
    padding: var(--sp-2) var(--sp-5);
    background: var(--bg-elevated);
    border-top: 1px solid var(--border);
    color: var(--fg-secondary);
    font-size: 11px;
    user-select: none;
  }
</style>
