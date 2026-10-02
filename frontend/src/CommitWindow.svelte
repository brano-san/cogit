<script lang="ts">
  import { untrack } from "svelte";
  import Checkbox from "$components/common/Checkbox.svelte";
  import Caret from "$components/common/Caret.svelte";
  import ConfirmDialog from "$components/common/ConfirmDialog.svelte";
  import Dialog from "$components/common/Dialog.svelte";
  import Notifications from "$components/layout/Notifications.svelte";
  import Radio from "$components/common/Radio.svelte";
  import TooltipLayer from "$components/common/TooltipLayer.svelte";
  import VirtualList from "$components/common/VirtualList.svelte";
  import { installChildWindow } from "$lib/child-window";
  import { draftToSave, initialMessage, messageAfterCommit } from "$lib/commit-draft";
  import {
    COUNTER_INFO,
    amendMessage,
    counter,
    draftKeyOf,
    fullMessage,
    listedFiles,
    menuLabel,
    parseCommitWindow,
    planCommit,
    ready,
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
    commitTemplate,
    isPublished,
    listRemotes,
    recentCommits,
    repoRefs,
    type CommitDetails,
  } from "$lib/ipc";
  import { pushTo } from "$lib/ipc/ref-ops";
  import { ON_MAC, primary } from "$lib/platform";
  import { publishedOrAssume } from "$lib/published";
  import { menuPush } from "$lib/push-to";
  import { placePopup, type Box, type Placed } from "$lib/popup-place";
  import { followSettings } from "$lib/settings-sync";
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
  let asking = $state<{ pushAfter: boolean } | null>(null);
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
  let loaded = $state(false);

  const files = $derived(listedFiles(mode, worktree.staged, worktree.unstaged));
  const rows = $derived(sortedFiles(files, sortKey, descending));
  const selected = $derived(files.filter((file) => ticked.has(file.path)));
  const allTicked = $derived(files.length > 0 && selected.length === files.length);
  const last = $derived(history[0] ? fullMessage(history[0]) : null);
  const recent = $derived(recentMessages(history));
  const canCommit = $derived(
    ready({
      message,
      template,
      ticked: selected.length,
      stagedTotal: worktree.staged.length,
      amend,
      unborn,
      busy: worktree.loading || !loaded,
      committing,
    }),
  );

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
        mode = worktree.staged.length > 0 ? "staged" : "local";
        tickAll(listedFiles(mode, worktree.staged, worktree.unstaged).map((file) => file.path));
        loaded = true;
        field?.focus();
      })();
    }),
  );

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



  async function start(pushAfter: boolean) {
    if (!canCommit || !request) return;
    if (amend && !unborn && (await publishedOrAssume(isPublished(request.repo, "HEAD")))) {
      asking = { pushAfter };
      return;
    }
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
      );
      if (plan.stage.length > 0) await worktree.stage(request.repo, plan.stage);
      await worktree.commit(request.repo, message, amend, noVerify, plan.only, signoff);
    } catch (err) {
      // A hook's refusal or git's own words: the window stays, the message with it.
      notices.report(err, "Could not commit");
      return;
    } finally {
      committing = false;
    }
    writeDraft(null);
    message = messageAfterCommit(template);
    if (pushAfter && !(await push(request.repo))) return;
    void closeThisWindow();
  }

  /** The branch's own push, as the ref menu's Push would do it; false when it failed. */
  async function push(repo: number): Promise<boolean> {
    try {
      const [refs, remotes] = await Promise.all([repoRefs(repo), listRemotes(repo)]);
      const head = refs.head;
      if (head.kind !== "branch") throw new Error("Committed, but HEAD is not on a branch, so nothing was pushed.");
      const branch = refs.branches.find((entry) => entry.kind === "local" && entry.name === head.name);
      const primary = remotes.includes("origin") ? "origin" : (remotes[0] ?? null);
      const plan = menuPush(
        { kind: "branch", name: head.name, upstream: branch?.upstream ?? null, remote: branch?.pushRemote ?? undefined },
        remotes,
        primary,
      );
      if (!plan) throw new Error("Committed, but this repository has no remote to push to.");
      await pushTo(repo, plan.remote, plan.refspec, plan.track, () => {});
      return true;
    } catch (err) {
      notices.report(err, "Could not push");
      return false;
    }
  }

  function onkeydown(event: KeyboardEvent) {
    if (asking || picking) return;
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

<svelte:window {onkeydown} onresize={() => (menuAt = null)} />

<TooltipLayer />

{#if !request}
  <p class="note">This window needs a repository. Open it with Ctrl+K in the main window.</p>
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

      <div class="table">
        <div class="thead">
          <span class="cell check"
            ><Checkbox
              checked={allTicked}
              ariaLabel="Select all files"
              onchange={(on) => tickAll(on ? files.map((file) => file.path) : [])}
            /></span
          >
          {#each [["name", "Name"], ["state", "State"], ["directory", "Directory"]] as [key, title] (key)}
            {#if key === "state"}
              <span class="cell head-button">{title}</span>
            {:else}
              <button type="button" class="cell head-button" onclick={() => sortBy(key as SortKey)}>
                {title}
                {#if sortKey === key}<span class="arrow" class:flipped={!descending}><Caret /></span>{/if}
              </button>
            {/if}
          {/each}
        </div>
        {#if rows.length === 0}
          <p class="empty">{worktree.loading || !loaded ? "Reading changes…" : "No files to commit."}</p>
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
      <textarea bind:this={field} bind:value={message} aria-label="Commit message" spellcheck="false"></textarea>

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
      <button type="button" class="primary" disabled={!canCommit} onclick={() => void start(false)}>Commit</button>
      <button type="button" disabled={!canCommit || unborn} onclick={() => void start(true)}>Commit &amp; Push</button>
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

  {#if asking}
    <ConfirmDialog
      title="Amend a Published Commit"
      message="This commit is already on a remote. Amending it gives it a new id, so the branch will need a force-push and anyone who pulled it will have to reset. Continue?"
      confirm="Amend"
      warning
      onanswer={(yes) => {
        const go = asking;
        asking = null;
        if (yes && go) void run(go.pushAfter);
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
    font-size: 10px;
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
    grid-template-columns: 28px minmax(120px, 1fr) minmax(90px, 0.9fr) minmax(120px, 1.5fr);
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

  .head-button {
    height: 100%;
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
    font-size: 11px;
  }

  .state.danger {
    color: var(--status-danger);
  }

  .empty,
  .note {
    margin: var(--sp-5);
    color: var(--text-secondary);
  }

  .label {
    flex: 1 1 auto;
    font-weight: 600;
  }

  textarea {
    display: block;
    flex: 2 1 0;
    box-sizing: border-box;
    width: 100%;
    min-height: 90px;
    resize: none;
    padding: var(--sp-3);
    background: var(--surface-input);
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
