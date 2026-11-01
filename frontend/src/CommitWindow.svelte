<script lang="ts">
  import EmptyState from "$components/common/EmptyState.svelte";
  import { untrack } from "svelte";
  import Checkbox from "$components/common/Checkbox.svelte";
  import Caret from "$components/common/Caret.svelte";
  import ConfirmDialog from "$components/common/ConfirmDialog.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import Notifications from "$components/layout/Notifications.svelte";
  import Radio from "$components/common/Radio.svelte";
  import TooltipLayer from "$components/common/TooltipLayer.svelte";
  import VirtualList from "$components/common/VirtualList.svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { installChildWindow, onMenuAction } from "$lib/child-window";
  import { draftToSave, initialMessage, messageAfterCommit } from "$lib/commit-draft";
  import {
    COUNTER_INFO,
    amendMessage,
    counter,
    draftKeyOf,
    fullMessage,
    listedFiles,
    menuLabel,
    opening,
    COLUMN_WIDTHS_KEY,
    DEFAULT_COLUMN_WIDTHS,
    columnWidth,
    leftOut,
    parseColumnWidths,
    type ColumnWidths,
    type ResizedColumn,
    parseCommitWindow,
    partialQuestion,
    planCommit,
    readyBlocker,
    recentMessages,
    sortedFiles,
    splitPath,
    stateSide,
    type CommitMode,
    type SortKey,
  } from "$lib/commit-window";
  import FileStateIcon from "$components/common/FileStateIcon.svelte";
  import { fileState } from "$lib/file-state";
  import { shortOid } from "$lib/format";
  import {
    closeThisWindow,
    onRepoChanged,
    repoWritten,
    commitTemplate,
    isPublished,
    popupContextMenu,
    recentCommits,
    spellingMisspelled,
    spellingSuggestions,
    repoRefs,
    type CommitDetails,
    type SpellLanguage,
  } from "$lib/ipc";
  import { ON_MAC, primary } from "$lib/platform";
  import { publishedOrAssume } from "$lib/published";
  import { requestPush } from "$lib/push-head";
  import { placePopup, type Box, type Placed } from "$lib/popup-place";
  import { followSettings } from "$lib/settings-sync";
  import { SPELL_ADD, SPELL_USE, accepted, mirrorHtml, spellLanguages, spellMenu, spellWords, type SpellWord } from "$lib/spelling";
  import type { ConfirmRequest } from "$stores/confirm.svelte";
  import { notices } from "$stores/notices.svelte";
  import { settings } from "$stores/settings.svelte";
  import { worktree } from "$stores/worktree.svelte";

  const request = parseCommitWindow(window.location.search);
  const draftKey = request ? draftKeyOf(request.root) : "";

  $effect(() => installChildWindow(window));
  $effect(() => followSettings(() => void settings.reload()));

  let mode = $state<CommitMode>("staged");
  let ticked = $state.raw<ReadonlySet<string>>(new Set());
  let sortKey = $state<SortKey>("name");
  let descending = $state(false);
  let message = $state("");
  let template = $state<string | null>(null);
  let amend = $state(false);
  let before = $state<string | null>(null);
  let more = $state(false);
  let signoff = $state(false);
  let noVerify = $state(false);
  let unborn = $state(false);
  let history = $state.raw<CommitDetails[]>([]);
  let committing = $state(false);
  /** The question the window waits on, answered through `ask`. */
  let question = $state<(ConfirmRequest & { settle: (yes: boolean) => void }) | null>(null);
  const ask = (request: ConfirmRequest) => new Promise<boolean>((settle) => (question = { ...request, settle }));
  const AMEND_PUBLISHED: ConfirmRequest = {
    title: "Amend a Published Commit",
    message:
      "This commit is already on a remote. Amending it gives it a new id, so the branch will need a force-push and anyone who pulled it will have to reset. Continue?",
    confirm: "Amend",
    warning: true,
  };
  let picking = $state(false);
  let chosen = $state<string | null>(null);
  let pending = $state<string | null>(null);
  let menuAt = $state<Box | null>(null);
  let placed = $state<Placed | null>(null);

  /** Moves the node to <body>, out of any clipping or stacking ancestor. */
  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return { destroy: () => node.remove() };
  }

  /** The menu is measured once it is in <body>, then placed inside the window. */
  function place(node: HTMLElement) {
    if (menuAt) {
      placed = placePopup(menuAt, { width: node.offsetWidth, height: node.scrollHeight + 2 }, {
        width: window.innerWidth,
        height: window.innerHeight,
      });
    }
    return { destroy: () => (placed = null) };
  }
  let field: HTMLTextAreaElement | undefined = $state();
  let mirror: HTMLDivElement | undefined = $state();
  const SPELL_PAUSE_MS = 250;
  let misses = $state.raw<ReadonlySet<string>>(new Set());
  let spellAt: { start: number; word: string; suggestions: string[] } | null = null;
  let spellRun = 0;
  let spelledFor = "";
  const spelled = new Map<string, boolean>();
  const languages = $derived(spellLanguages(settings.current));
  const words = $derived(spellWords(message));
  const mirrored = $derived(mirrorHtml(message, words, misses));

  $effect(() => {
    const chosen = languages;
    const personal = settings.current.spellWords;
    const found = words;
    const timer = setTimeout(() => void spell(chosen, personal, found), SPELL_PAUSE_MS);
    return () => clearTimeout(timer);
  });

  $effect(() => {
    void mirrored;
    syncMirror();
  });

  $effect(() => {
    if (!field) return;
    const watcher = new ResizeObserver(syncMirror);
    watcher.observe(field);
    return () => watcher.disconnect();
  });

  $effect(() =>
    onMenuAction(window, (action) => {
      const at = spellAt;
      spellAt = null;
      if (!at) return;
      if (action === SPELL_ADD) {
        void settings.apply({ ...settings.current, spellWords: [...settings.current.spellWords, at.word] });
      } else if (action.startsWith(SPELL_USE)) {
        const replacement = at.suggestions[Number(action.slice(SPELL_USE.length))];
        if (replacement !== undefined) replaceWord(at.start, at.word, replacement);
      }
    }),
  );

  async function spell(chosen: SpellLanguage[], personal: readonly string[], found: readonly SpellWord[]) {
    const run = ++spellRun;
    const key = chosen.join(",");
    if (key !== spelledFor) {
      spelled.clear();
      spelledFor = key;
    }
    const unknown = [...new Set(found.map((one) => one.word))].filter(
      (word) => !spelled.has(word) && !accepted(word, personal),
    );
    if (chosen.length > 0 && unknown.length > 0) {
      let wrong: Set<string>;
      try {
        wrong = new Set(await spellingMisspelled(chosen, unknown));
      } catch (err) {
        notices.report(err, "Could not check the spelling");
        return;
      }
      if (run !== spellRun) return;
      for (const word of unknown) spelled.set(word, wrong.has(word));
    }
    misses = new Set(
      found.map((one) => one.word).filter((word) => chosen.length > 0 && spelled.get(word) === true && !accepted(word, personal)),
    );
  }

  function syncMirror() {
    if (!mirror || !field) return;
    mirror.scrollTop = field.scrollTop;
    mirror.style.paddingRight = `calc(var(--sp-3) + ${field.offsetWidth - field.clientWidth - 2}px)`;
  }

  function replaceWord(start: number, word: string, replacement: string) {
    if (!field || message.slice(start, start + word.length) !== word) return;
    field.focus();
    field.setSelectionRange(start, start + word.length);
    if (!document.execCommand("insertText", false, replacement)) {
      message = message.slice(0, start) + replacement + message.slice(start + word.length);
    }
  }

  async function onFieldMenu(event: MouseEvent) {
    const hit = [...(mirror?.querySelectorAll<HTMLElement>(".miss") ?? [])].find((span) =>
      [...span.getClientRects()].some(
        (box) => event.clientX >= box.left && event.clientX <= box.right && event.clientY >= box.top && event.clientY <= box.bottom,
      ),
    );
    if (!hit) return;
    event.preventDefault();
    const word = hit.textContent ?? "";
    const suggestions = await spellingSuggestions(languages, word).catch((err: unknown) => {
      notices.report(err, "Could not read the spelling suggestions");
      return [];
    });
    spellAt = { start: Number(hit.dataset.at), word, suggestions };
    await popupContextMenu(spellMenu(getCurrentWindow().label, word, suggestions), event.clientX, event.clientY).catch(
      (err: unknown) => notices.report(err, "Could not show the spelling suggestions"),
    );
  }
  let loaded = $state(false);

  const files = $derived(listedFiles(mode, worktree.staged, worktree.unstaged));
  const rows = $derived(sortedFiles(files, sortKey, descending));
  const selected = $derived(files.filter((file) => ticked.has(file.path)));
  const allTicked = $derived(files.length > 0 && selected.length === files.length);
  const last = $derived(history[0] ? fullMessage(history[0]) : null);
  const recent = $derived(recentMessages(history));
  const blocked = $derived(
    readyBlocker({
      message,
      template,
      ticked: selected.length,
      stagedTotal: worktree.staged.length,
      amend,
      unborn,
      busy: worktree.loading || !loaded,
      committing,
      conflicted: selected.filter((file) => file.status === "conflicted").length,
    }),
  );
  const canCommit = $derived(blocked === null);

  let widths = $state(readColumnWidths());

  function readColumnWidths(): ColumnWidths {
    try {
      return parseColumnWidths(localStorage.getItem(COLUMN_WIDTHS_KEY));
    } catch {
      return { ...DEFAULT_COLUMN_WIDTHS };
    }
  }

  function startResize(key: ResizedColumn, event: PointerEvent) {
    event.preventDefault();
    const startX = event.clientX;
    const startWidth = widths[key];
    const move = (moved: PointerEvent) => {
      widths = { ...widths, [key]: columnWidth(startWidth + moved.clientX - startX) };
    };
    const stop = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", stop);
      window.removeEventListener("pointercancel", stop);
      try {
        localStorage.setItem(COLUMN_WIDTHS_KEY, JSON.stringify(widths));
      } catch {
        return;
      }
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", stop);
    window.addEventListener("pointercancel", stop);
  }

  function tickAll(paths: readonly string[]) {
    ticked = new Set(paths);
  }

  function setMode(next: CommitMode) {
    if (next === mode) return;
    mode = next;
    tickAll(listedFiles(next, worktree.staged, worktree.unstaged).map((file) => file.path));
  }

  function toggle(path: string) {
    const next = new Set(ticked);
    if (!next.delete(path)) next.add(path);
    ticked = next;
  }

  function sortBy(key: SortKey) {
    if (sortKey === key) descending = !descending;
    else {
      sortKey = key;
      descending = false;
    }
  }

  function readDraft(): string | null {
    try {
      return localStorage.getItem(draftKey);
    } catch {
      return null;
    }
  }

  function writeDraft(value: string | null) {
    try {
      if (value === null) localStorage.removeItem(draftKey);
      else localStorage.setItem(draftKey, value);
    } catch {
      // A blocked store costs the shared draft, not the commit.
    }
  }

  // Once, on opening: what the loads read must not make this effect run them again.
  $effect(() =>
    untrack(() => {
      if (!request) return;
      void settings.load();
      void (async () => {
        const id = request.repo;
        const [text, refs, log] = await Promise.all([
          commitTemplate(id).catch(() => null),
          repoRefs(id).catch(() => null),
          recentCommits(id, 100).catch(() => [] as CommitDetails[]),
          worktree.load(id),
        ]);
        template = text ?? null;
        unborn = refs?.head.kind === "unborn";
        history = log;
        message = initialMessage(readDraft(), template);
        const start = opening(request.paths, worktree.staged, worktree.unstaged);
        mode = start.mode;
        tickAll(start.ticked);
        loaded = true;
        field?.focus();
      })();
    }),
  );

  async function reread() {
    if (!request || !loaded || committing) return;
    const id = request.repo;
    const [log] = await Promise.all([recentCommits(id, 100).catch(() => history), worktree.load(id)]);
    history = log;
    const listed = new Set(listedFiles(mode, worktree.staged, worktree.unstaged).map((file) => file.path));
    ticked = new Set([...ticked].filter((path) => listed.has(path)));
  }

  $effect(() => {
    if (!request) return;
    const stop = onRepoChanged((change) => {
      if (change.repo === request.repo) void reread();
    });
    return () => void stop.then((undo) => undo()).catch(() => {});
  });

  // The inline panel keeps the same draft: it is written on every change, and read back
  // when the other window changes it. Amend's borrowed message is nobody's draft.
  $effect(() => {
    if (!loaded || amend) return;
    writeDraft(draftToSave(message, template));
  });
  $effect(() => {
    const onstorage = (event: StorageEvent) => {
      if (event.key !== draftKey || amend) return;
      const next = initialMessage(event.newValue, template);
      if (next !== message) message = next;
    };
    window.addEventListener("storage", onstorage);
    return () => window.removeEventListener("storage", onstorage);
  });

  function onAmend(checked: boolean) {
    if (checked && last === null) {
      amend = false;
      return;
    }
    const next = amendMessage(checked, message, last ?? "", before);
    message = next.message;
    before = next.before;
  }

  function useMessage(text: string) {
    message = text;
    menuAt = null;
    field?.focus();
  }

  function openMenu(event: MouseEvent) {
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    menuAt = box;
    pending = readDraft();
  }


  /** Changes left out are asked about first, unless Amend only rewords (nothing ticked). */
  async function start(pushAfter: boolean) {
    if (!canCommit || !request) return;
    const out = leftOut(mode, ticked, worktree.staged, worktree.unstaged);
    if (selected.length > 0 && out.length > 0 && !(await ask(partialQuestion(out, pushAfter)))) return;
    const published = amend && !unborn && (await publishedOrAssume(isPublished(request.repo, "HEAD")));
    if (published && !(await ask(AMEND_PUBLISHED))) return;
    await run(pushAfter);
  }

  async function run(pushAfter: boolean) {
    if (!request) return;
    committing = true;
    try {
      const plan = planCommit(
        mode,
        selected.map((file) => file.path),
        worktree.staged,
        worktree.unstaged,
      );
      if (plan.stage.length > 0) await worktree.stage(request.repo, plan.stage, true);
      await worktree.commit(request.repo, message, amend, noVerify, plan.only, signoff);
    } catch (err) {
      // A hook's refusal or git's own words: the window stays, the message with it.
      notices.report(err, "Could not commit");
      return;
    } finally {
      committing = false;
      // Staged or committed: the main window learns it only from here (R-25).
      void repoWritten(request.repo);
    }
    // Amend committed the last commit's message; the stored draft is the next one's.
    if (!amend) writeDraft(null);
    message = messageAfterCommit(template);
    if (pushAfter) await requestPush(request.repo);
    void closeThisWindow();
  }

  function onkeydown(event: KeyboardEvent) {
    if (question || picking) return;
    if (event.key === "Enter" && primary(event, ON_MAC)) {
      event.preventDefault();
      void start(false);
    } else if (event.key === "Escape" && menuAt) {
      event.preventDefault();
      menuAt = null;
    }
  }

  const date = (details: CommitDetails) =>
    settings.formatDate(details.author.timestamp, details.author.tzOffsetMinutes);
</script>

<svelte:window onfocus={() => void reread()} {onkeydown} onresize={() => (menuAt = null)} />

<TooltipLayer />

{#if !request}
  <EmptyState title="This window needs a repository. Open it with Ctrl+K in the main window." />
{:else}
  <div class="window">
    <header class="head">
      <h1>Commit local or staged changes</h1>
      <p>Select the files you want to commit and provide a commit message.</p>
    </header>

    <div class="body">
      <div class="modes">
        <Radio
          name="commit-mode"
          label="Staged Changes"
          checked={mode === "staged"}
          onchange={() => setMode("staged")}
        />
        <Radio
          name="commit-mode"
          label="Local Changes"
          checked={mode === "local"}
          onchange={() => setMode("local")}
        />
        <span class="grow"></span>
        <span class="count tabular">{counter(selected)}</span>
        <span class="info" title={COUNTER_INFO} role="img" aria-label="About the file counts">i</span>
      </div>

      <div class="table" style:--name-width="{widths.name}px" style:--state-width="{widths.state}px">
        <div class="thead">
          <span class="cell check"
            ><Checkbox
              checked={allTicked}
              ariaLabel="Select all files"
              onchange={(on) => tickAll(on ? files.map((file) => file.path) : [])}
            /></span
          >
          {#each [["name", "Name"], ["state", "State"], ["directory", "Directory"]] as [key, title] (key)}
            <div class="cell header">
              {#if key === "state"}
                <span class="head-button">{title}</span>
              {:else}
                <button type="button" class="head-button" onclick={() => sortBy(key as SortKey)}>
                  {title}
                  {#if sortKey === key}<span class="arrow" class:flipped={!descending}><Caret /></span>{/if}
                </button>
              {/if}
              {#if key === "name" || key === "state"}
                <!-- svelte-ignore a11y_interactive_supports_focus -->
                <span
                  class="resizer"
                  role="separator"
                  aria-orientation="vertical"
                  aria-label="Resize the {title} column"
                  onpointerdown={(event) => startResize(key as ResizedColumn, event)}
                ></span>
              {/if}
            </div>
          {/each}
        </div>
        {#if rows.length === 0}
          <EmptyState title={worktree.loading || !loaded ? "Reading changes…" : "No files to commit."} />
        {:else}
          <VirtualList items={rows} label="Files to commit">
            {#snippet row(file, at)}
              {@const parts = splitPath(file.path)}
              {@const state = fileState(file, stateSide(mode, file.path, worktree.staged, worktree.unstaged))}
              <div class="row" style:top="{at * 24}px" class:on={ticked.has(file.path)}>
                <span class="cell check"
                  ><Checkbox
                    checked={ticked.has(file.path)}
                    ariaLabel={file.path}
                    onchange={() => toggle(file.path)}
                  /></span
                >
                <span class="cell name" title={file.path}>
                  <FileStateIcon base={file.mode === "submodule" ? "repository" : "page"} state={state.icon} />
                  <span class="truncate">{parts.name}</span>
                </span>
                <span class="cell state truncate" class:danger={state.tone === "danger"} title={state.tooltip}
                  >{state.text}</span
                >
                <span class="cell dir truncate" title={parts.directory}>{parts.directory}</span>
              </div>
            {/snippet}
          </VirtualList>
        {/if}
      </div>

      <div class="label-row">
        <span class="label">Commit Message</span>
        <button type="button" class="select" aria-haspopup="menu" onclick={openMenu}>
          Select <Caret open={menuAt !== null} />
        </button>
      </div>
      <div class="message">
        <div class="spell" aria-hidden="true" bind:this={mirror}>{@html mirrored}</div>
        <textarea
          bind:this={field}
          bind:value={message}
          aria-label="Commit message"
          spellcheck="false"
          onscroll={syncMirror}
          oncontextmenu={(event) => void onFieldMenu(event)}
        ></textarea>
      </div>

      <div class="options">
        <Checkbox
          checked={amend}
          label="Amend last commit"
          disabled={unborn || last === null}
          title={unborn
            ? "Nothing to amend: this branch has no commits yet"
            : "Replaces the last commit with a new one made from its message and the checked files. The commit gets a new id."}
          onchange={(on) => {
            amend = on;
            onAmend(on);
          }}
        />
        <Checkbox bind:checked={more} label="More Options" />
        {#if more}
          <div class="sub">
            <Checkbox bind:checked={signoff} label="Add 'Signed-off-by' signature" />
            <Checkbox bind:checked={noVerify} label="Bypass commit hook (--no-verify)" />
          </div>
        {/if}
      </div>
    </div>

    <footer class="buttons">
      <span class="grow"></span>
      <button type="button" class="primary" disabled={!canCommit} title={blocked ?? undefined} onclick={() => void start(false)}
        >Commit</button
      >
      <button
        type="button"
        disabled={!canCommit || unborn}
        title={blocked ?? (unborn ? "This branch has no commits yet: Commit, then Push" : undefined)}
        onclick={() => void start(true)}>Commit &amp; Push</button
      >
      <button type="button" onclick={() => void closeThisWindow()}>Cancel</button>
    </footer>
  </div>

  {#if menuAt}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="backdrop" use:portal onclick={() => (menuAt = null)}></div>
    <div
      class="menu"
      role="menu"
      use:portal
      use:place
      style:visibility={placed ? "visible" : "hidden"}
      style:left="{placed?.left ?? 0}px"
      style:top="{placed?.top ?? 0}px"
      style:max-height="{placed?.maxHeight ?? 0}px"
    >
      <button
        type="button"
        role="menuitem"
        onclick={() => {
          menuAt = null;
          picking = true;
        }}>Select from Log…</button
      >
      <hr />
      <button
        type="button"
        role="menuitem"
        disabled={pending === null}
        title={pending === null ? "No message was saved" : undefined}
        onclick={() => useMessage(initialMessage(pending, template))}>Pending Message</button
      >
      {#if recent.length > 0}<hr />{/if}
      {#each recent as text (text)}
        <button type="button" role="menuitem" title={text} onclick={() => useMessage(text)}>
          <span class="truncate">{menuLabel(text)}</span>
        </button>
      {/each}
    </div>
  {/if}

  {#if picking}
    <Dialog
      title="Select from Log"
      width="min(640px, 92vw)"
      height="min(480px, 80vh)"
      flush
      onclose={() => (picking = false)}
      onconfirm={() => {
        const found = history.find((entry) => entry.oid === chosen);
        if (found) useMessage(fullMessage(found));
        picking = false;
      }}
    >
      <div class="log" role="listbox" aria-label="Commits">
        {#each history as entry (entry.oid)}
          <button
            type="button"
            role="option"
            aria-selected={chosen === entry.oid}
            class:on={chosen === entry.oid}
            onclick={() => (chosen = entry.oid)}
            ondblclick={() => {
              useMessage(fullMessage(entry));
              picking = false;
            }}
          >
            <span class="oid mono">{shortOid(entry.oid)}</span>
            <span class="truncate">{entry.summary}</span>
            <span class="who truncate">{entry.author.name} · {date(entry)}</span>
          </button>
        {/each}
      </div>
      {#snippet footer()}
        <button class="btn" type="button" onclick={() => (picking = false)}>Cancel</button>
        <button
          class="btn primary"
          type="button"
          disabled={chosen === null}
          onclick={() => {
            const found = history.find((entry) => entry.oid === chosen);
            if (found) useMessage(fullMessage(found));
            picking = false;
          }}>Select</button
        >
      {/snippet}
    </Dialog>
  {/if}

  {#if question}
    {@const asked = question}
    <ConfirmDialog
      title={asked.title}
      message={asked.message}
      confirm={asked.confirm}
      warning={asked.warning}
      items={asked.items}
      onanswer={(yes) => {
        // `asked` is derived from `question`: settle before it reads null.
        asked.settle(yes);
        question = null;
      }}
    />
  {/if}

  <Notifications onopenurl={() => {}} onshowoutput={() => {}} onaction={() => {}} />
{/if}

<style>
  .window {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--surface-base);
    color: var(--text-primary);
    font-family: var(--font-ui);
    font-size: var(--fs-dense);
  }

  .head {
    flex: none;
    padding: var(--sp-5) var(--sp-6);
    background: var(--surface-panel);
    border-bottom: 1px solid var(--divider);
  }

  h1 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }

  .head p {
    margin: var(--sp-2) 0 0;
    color: var(--text-secondary);
  }

  .body {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    gap: var(--sp-4);
    min-height: 0;
    padding: var(--sp-5) var(--sp-6);
  }

  .modes,
  .label-row,
  .buttons {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--sp-6);
  }

  .grow {
    flex: 1 1 auto;
  }

  .count {
    color: var(--text-secondary);
  }

  .info {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    border: 1px solid var(--text-secondary);
    border-radius: 50%;
    color: var(--text-secondary);
    font-size: var(--fs-small);
    font-style: italic;
    cursor: default;
  }

  .table {
    display: flex;
    flex: 3 1 0;
    flex-direction: column;
    min-height: 120px;
    background: var(--surface-input);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
  }

  .thead,
  .row {
    display: grid;
    grid-template-columns: 28px var(--name-width) var(--state-width) minmax(120px, 1fr);
    align-items: center;
    height: 24px;
  }

  .thead {
    flex: none;
    background: var(--surface-panel);
    border-bottom: 1px solid var(--divider);
  }

  .row {
    position: absolute;
    left: 0;
    right: 0;
  }

  .row:hover {
    background: var(--state-hover);
  }

  .cell {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-width: 0;
    padding: 0 var(--sp-3);
  }

  .check {
    justify-content: center;
  }

  .header {
    position: relative;
    height: 100%;
    min-width: 0;
  }

  .resizer {
    position: absolute;
    top: 0;
    right: -3px;
    bottom: 0;
    width: 6px;
    z-index: 1;
    cursor: col-resize;
    touch-action: none;
  }

  .resizer:hover {
    background: var(--splitter-active);
  }

  .head-button {
    display: flex;
    align-items: center;
    width: 100%;
    height: 100%;
    padding: 0 var(--sp-3);
    background: none;
    border: 0;
    border-left: 1px solid var(--divider);
    color: var(--text-secondary);
    font: inherit;
    text-align: left;
    cursor: default;
  }

  .arrow {
    display: inline-flex;
  }

  .arrow.flipped {
    transform: rotate(180deg);
  }

  .dir {
    color: var(--text-secondary);
  }

  .truncate {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .state {
    color: var(--fg-secondary);
    font-size: var(--fs-header);
  }

  .state.danger {
    color: var(--status-danger);
  }

  .label {
    flex: 1 1 auto;
    font-weight: 600;
  }

  .message {
    position: relative;
    display: flex;
    flex: 2 1 0;
    min-height: 90px;
  }

  .spell {
    position: absolute;
    inset: 0;
    box-sizing: border-box;
    padding: var(--sp-3);
    overflow: hidden;
    background: var(--surface-input);
    color: transparent;
    border: 1px solid transparent;
    border-radius: var(--r-sm);
    font-family: var(--font-mono);
    font-size: var(--fs-code);
    line-height: var(--lh-code);
    white-space: pre-wrap;
    overflow-wrap: break-word;
    pointer-events: none;
  }

  .spell :global(.miss) {
    text-decoration: underline wavy var(--status-danger);
    text-decoration-skip-ink: none;
  }

  textarea {
    position: relative;
    display: block;
    flex: 1 1 auto;
    box-sizing: border-box;
    width: 100%;
    min-height: 0;
    resize: none;
    padding: var(--sp-3);
    background: transparent;
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-family: var(--font-mono);
    font-size: var(--fs-code);
    line-height: var(--lh-code);
  }

  .options {
    display: flex;
    flex: none;
    flex-direction: column;
    gap: var(--sp-3);
  }

  .sub {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding-left: var(--sp-7);
  }

  .buttons {
    justify-content: flex-end;
    gap: var(--sp-4);
    padding: var(--sp-4) var(--sp-6);
    border-top: 1px solid var(--divider);
    background: var(--surface-panel);
  }

  .buttons button,
  .select {
    height: 24px;
    padding: 0 var(--sp-5);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font: inherit;
    cursor: default;
  }

  .select {
    display: inline-flex;
    align-items: center;
    gap: var(--sp-3);
  }

  .buttons button.primary {
    border-color: var(--status-ref);
  }

  .buttons button:disabled {
    opacity: 0.45;
  }

  .buttons button:not(:disabled):hover,
  .select:hover {
    border-color: var(--status-ref);
  }

  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 30;
  }

  .menu {
    position: fixed;
    z-index: 31;
    display: flex;
    flex-direction: column;
    min-width: 260px;
    max-width: 420px;
    overflow: auto;
    padding: var(--sp-2) 0;
    background: var(--surface-raised);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    box-shadow: var(--shadow-popover);
  }

  .menu button {
    display: flex;
    align-items: center;
    flex: none;
    height: var(--h-row);
    padding: 0 var(--sp-5);
    background: none;
    color: var(--text-primary);
    border: 0;
    font: inherit;
    text-align: left;
    cursor: default;
  }

  .menu button:hover:not(:disabled) {
    background: var(--state-hover);
  }

  .menu button:disabled {
    opacity: 0.4;
  }

  .menu hr {
    width: 100%;
    margin: var(--sp-2) 0;
    border: 0;
    border-top: 1px solid var(--divider);
  }

  .log {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: auto;
  }

  .log button {
    display: grid;
    grid-template-columns: 64px 1fr minmax(0, 200px);
    gap: var(--sp-4);
    flex: none;
    align-items: center;
    height: 24px;
    padding: 0 var(--sp-5);
    background: none;
    color: var(--text-primary);
    border: 0;
    font: inherit;
    text-align: left;
    cursor: default;
  }

  .log button:hover {
    background: var(--state-hover);
  }

  .log button.on {
    background: var(--state-selected);
  }

  .oid,
  .who {
    color: var(--text-secondary);
  }
</style>
