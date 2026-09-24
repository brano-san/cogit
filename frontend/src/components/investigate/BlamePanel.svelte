<script lang="ts">
  import { shortOid } from "$lib/format";
  import Avatar from "$components/common/Avatar.svelte";
  import VirtualList from "$components/common/VirtualList.svelte";
  import OriginCard from "./OriginCard.svelte";
  import { highlightLines, mergePieces } from "$lib/highlight";
  import {
    ageOf,
    commitOf,
    dateOf,
    languageOf,
    markerOf,
    nextChange,
    shortAuthor,
  } from "$lib/investigate/blame";
  import type { InvestigateSession } from "$lib/investigate/session.svelte";
  import type { OriginLine } from "$lib/ipc/investigate";

  interface Props {
    session: InvestigateSession;
    /** Unix seconds, for the age column. */
    now: number;
  }

  let { session, now }: Props = $props();

  const ROW = 18;
  const tables = $derived(session.blame);
  const tokens = $derived(
    tables ? highlightLines(tables.lines.map((line) => line.text), languageOf(session.location.path)) : [],
  );
  const own = $derived.by(() => {
    const indices = new Set<number>();
    for (const block of session.changes) {
      for (let at = block.start; at < block.end; at++) indices.add(at);
    }
    return indices;
  });
  const previous = $derived(nextChange(session.changes, session.selectedLine ?? -1, -1));
  const next = $derived(nextChange(session.changes, session.selectedLine ?? -1, 1));

  /** The version on screen, from its Navigation row or, failing that, from blame. */
  const version = $derived.by(() => {
    const item = session.items[session.selectedItem];
    if (item?.kind === "commit") {
      const row = item.row;
      return { oid: row.oid, author: row.author, email: row.email, timestamp: row.timestamp, summary: row.summary };
    }
    return null;
  });

  function introducedBy(line: OriginLine | undefined): string {
    if (!tables || !line) return "";
    const commit = commitOf(tables, line);
    if (!commit) return "";
    if (commit.uncommitted) return "not committed yet";
    return `${shortOid(commit.oid)} · ${commit.author} · ${ageOf(commit.timestamp, now)} ago`;
  }

  function startsRun(index: number): boolean {
    if (!tables || index === 0) return true;
    const here = tables.lines[index];
    const before = tables.lines[index - 1];
    return !here || !before || commitOf(tables, here) !== commitOf(tables, before);
  }

  function inBlock(index: number): boolean {
    const block = session.block;
    return !!block && index >= block.start && index < block.end;
  }

  function onkeydown(event: KeyboardEvent) {
    if (!tables || event.altKey || event.ctrlKey) return;
    const at = session.selectedLine ?? -1;
    if (event.key === "ArrowDown" && at < tables.lines.length - 1) session.selectLine(at + 1);
    else if (event.key === "ArrowUp" && at > 0) session.selectLine(at - 1);
    else return;
    event.preventDefault();
  }

  /** The origin card starts past the hash, marker, author, age and number columns. */
  const CARD_LEFT = "calc(7ch + 3ch + 10ch + 5ch + 6ch + 5 * var(--sp-3))";
</script>

<div class="panel">
  <header class="blame-for">
    <span class="caption">Blame for</span>
    {#if version}
      <Avatar name={version.author} email={version.email} size={28} />
    {/if}
    <dl>
      <dt>File</dt>
      <dd class="mono truncate" title={session.location.path}>{session.location.path}</dd>
      <dt>Commit</dt>
      <dd class="truncate">
        {#if version}
          <span class="mono">{shortOid(version.oid)}</span>
          {version.author}, {dateOf(version.timestamp)}
        {:else if session.location.rev === null}
          Working Tree
        {:else}
          <span class="mono">{shortOid(session.location.rev)}</span>
        {/if}
      </dd>
      <dt>Msg</dt>
      <dd class="truncate" title={version?.summary ?? ""}>
        {version?.summary ?? (session.location.rev === null ? "Uncommitted changes" : "")}
      </dd>
    </dl>
    <div class="arrows">
      <button
        type="button"
        disabled={previous === null}
        title="Previous change in this version (Shift+F6)"
        aria-label="Previous change"
        onclick={() => session.moveToChange(-1)}
        ><svg viewBox="0 0 16 16" aria-hidden="true"><path d="m3 10 5-5 5 5" /></svg></button
      >
      <button
        type="button"
        disabled={next === null}
        title="Next change in this version (F6)"
        aria-label="Next change"
        onclick={() => session.moveToChange(1)}
        ><svg viewBox="0 0 16 16" aria-hidden="true"><path d="m3 6 5 5 5-5" /></svg></button
      >
    </div>
  </header>

  {#if session.loadingBlame && !tables}
    <p class="note">Blaming {session.location.path}…</p>
  {:else if session.blameError}
    <p class="note error">{session.blameError}</p>
  {:else if tables}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div class="lines key-list" tabindex="0" role="listbox" aria-label="Lines with their origin" {onkeydown}>
      <VirtualList items={tables.lines} rowHeight={ROW} reveal={session.selectedLine}>
        {#snippet row(line: OriginLine, index: number)}
          {@const commit = commitOf(tables, line)}
          {@const first = startsRun(index)}
          {@const selected = index === session.selectedLine}
          {@const marker = markerOf(tables, line)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div
            class="line"
            class:selected
            class:block={inBlock(index)}
            class:own={own.has(index)}
            class:first
            role="option"
            aria-selected={selected}
            tabindex="-1"
            style:top="{index * ROW}px"
            onclick={() => session.selectLine(index)}
          >
            <span class="hash mono" title={commit?.summary}
              >{first && commit ? (commit.uncommitted ? "·······" : shortOid(commit.oid)) : ""}</span
            >
            <span
              class="marker mono"
              class:added={marker.endsWith("+")}
              class:merge={marker.startsWith("M")}
              title={marker.endsWith("+") ? "Added" : "Modified"}>{marker}</span
            >
            <span class="author truncate" title={commit?.author}
              >{first && commit ? shortAuthor(commit.author) : ""}</span
            >
            <span class="age tabular">{first && commit ? ageOf(commit.timestamp, now) : ""}</span>
            <span class="number tabular">{line.line}</span>
            <span class="code mono"
              >{#each mergePieces(line.text, tokens[index] ?? [], []) as piece, i (i)}<span
                  class={piece.cls}>{piece.text}</span
                >{/each}</span
            >
            {#if selected && session.cardOpen}
              <OriginCard {session} introduced={introducedBy(line)} left={CARD_LEFT} />
            {/if}
          </div>
        {/snippet}
      </VirtualList>
    </div>
  {/if}
</div>

<style>
  .panel {
    display: flex;
    flex: 1 1 auto;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--surface-base);
  }

  .blame-for {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--sp-5);
    padding: var(--sp-3) var(--sp-5);
    background: var(--surface-panel);
    border-bottom: 1px solid var(--divider);
    font-size: var(--fs-dense);
  }

  .caption {
    color: var(--text-secondary);
    white-space: nowrap;
  }

  dl {
    display: grid;
    flex: 1 1 auto;
    grid-template-columns: auto 1fr;
    column-gap: var(--sp-4);
    min-width: 0;
    margin: 0;
  }

  dt {
    color: var(--text-secondary);
  }

  dd {
    min-width: 0;
    margin: 0;
  }

  .arrows {
    display: flex;
    gap: var(--sp-2);
  }

  .arrows button {
    display: grid;
    place-items: center;
    width: var(--h-button-sm);
    height: var(--h-button-sm);
    padding: 0;
    background: none;
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    cursor: default;
  }

  .arrows button:hover:not(:disabled) {
    background: var(--state-hover);
  }

  .arrows button:disabled {
    opacity: 0.4;
  }

  .arrows svg {
    width: 12px;
    height: 12px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .note {
    margin: 0;
    padding: var(--sp-6);
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  .error {
    color: var(--status-delete);
    white-space: pre-wrap;
  }

  .lines {
    display: flex;
    flex: 1 1 auto;
    min-height: 0;
  }

  .line {
    position: absolute;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    height: 18px;
    padding: 0 var(--sp-3);
    font-size: var(--fs-code);
    white-space: pre;
    cursor: default;
  }

  .line.first {
    border-top: 1px solid var(--divider);
  }

  .line.own {
    background: var(--c-added-bg);
  }

  .line.block {
    background: var(--c-modified-bg);
  }

  .line:hover {
    background: var(--state-hover);
  }

  .line.selected {
    z-index: 4;
    background: var(--state-selected);
  }

  .hash {
    flex: 0 0 7ch;
    color: var(--text-secondary);
  }

  .marker {
    flex: 0 0 3ch;
    color: var(--status-modify);
    text-align: center;
  }

  .marker.added {
    color: var(--status-add);
  }

  .marker.merge {
    color: var(--status-ref);
  }

  .author {
    flex: 0 0 10ch;
    color: var(--text-secondary);
    font-family: var(--font-ui);
  }

  .age {
    flex: 0 0 5ch;
    color: var(--text-secondary);
    text-align: right;
  }

  .number {
    flex: 0 0 6ch;
    color: var(--text-secondary);
    text-align: right;
    user-select: none;
  }

  .code {
    flex: 1 1 auto;
    min-width: 0;
    overflow: hidden;
  }
</style>
