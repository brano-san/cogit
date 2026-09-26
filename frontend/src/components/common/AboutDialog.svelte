<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import { aboutGroups, diagnosticsText, type AboutRow, type Context } from "$lib/diagnostics";
  import { fitPath } from "$lib/truncate";
  import type { AppInfo } from "$lib/ipc";
  import type { UpdateOutcome } from "$lib/updates";
  import icon from "../../../../src-tauri/icons/128x128.png";

  /** Every number anyone asks for in a bug report, and a button that copies the lot. */
  interface Props {
    info: AppInfo;
    /** The last update check this session; null until one runs. */
    update: UpdateOutcome | null;
    onclose: () => void;
    oncopy: (text: string) => void;
    /** Shows the file in its folder, in the system's file manager. */
    onreveal: (path: string) => void;
    onlicences: () => void;
    oncheckupdates: () => void;
  }

  let { info, update, onclose, oncopy, onreveal, onlicences, oncheckupdates }: Props = $props();

  const COPIED_MS = 2000;

  const context: Context = $derived({
    svelte: __SVELTE_VERSION__,
    locale: navigator.language,
    update,
  });
  const groups = $derived(aboutGroups(info, context));
  const site = $derived(info.repository.replace(/^https?:\/\//, ""));

  let copied = $state(false);
  let closer: HTMLButtonElement | undefined = $state();
  let timer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    closer?.focus();
  });
  $effect(() => () => clearTimeout(timer));

  function copy() {
    oncopy(diagnosticsText(info, context));
    copied = true;
    clearTimeout(timer);
    timer = setTimeout(() => (copied = false), COPIED_MS);
  }

  /** The log row names the folder but reveals today's file in it. */
  function target(row: AboutRow): string {
    return row.label === "Log folder" ? info.logPath : row.value;
  }

  function folderHint(row: AboutRow): string {
    return row.label === "Log folder" ? "Open log folder" : "Open settings folder";
  }

  /** A click that ends a drag across the text is a selection, not a request to open. */
  function openUnlessSelecting(row: AboutRow) {
    if (window.getSelection()?.toString()) return;
    onreveal(target(row));
  }

  /** The row shows a shortened path; copying all of it copies the real one. */
  function copyWhole(event: ClipboardEvent, row: AboutRow) {
    const shown = (event.currentTarget as HTMLElement).textContent;
    if (window.getSelection()?.toString() !== shown) return;
    event.clipboardData?.setData("text/plain", row.value);
    event.preventDefault();
  }
</script>

<Dialog title="About Cogit" {onclose} width="min(580px, 92vw)">
  <div class="about">
    <header>
      <img src={icon} alt="" width="64" height="64" />
      <div>
        <p class="name">Cogit</p>
        <p class="muted">A Git client that says what it is doing.</p>
        <p class="muted">
          MIT license ·
          <a href={info.repository} target="_blank" rel="noreferrer">{site}</a>
          ·
          <button type="button" class="link" onclick={onlicences}>Third-party licenses</button>
        </p>
      </div>
    </header>

    {#each groups as group (group.title)}
      <section>
        <h3>{group.title}</h3>
        <dl>
          {#each group.rows as row (row.label)}
            <dt>{row.label}</dt>
            <dd>
              {#if row.path}
                <span
                  class="value path"
                  role="link"
                  tabindex="0"
                  title={row.value}
                  use:fitPath={row.value}
                  onclick={() => openUnlessSelecting(row)}
                  onkeydown={(event) => event.key === "Enter" && onreveal(target(row))}
                  oncopy={(event) => copyWhole(event, row)}
                ></span>
                <button
                  type="button"
                  class="folder"
                  title={folderHint(row)}
                  aria-label={folderHint(row)}
                  onclick={() => onreveal(target(row))}
                >
                  <svg viewBox="0 0 16 16" aria-hidden="true"
                    ><path
                      fill="currentColor"
                      d="M1.5 3.5c0-.69.56-1.25 1.25-1.25h3.04c.4 0 .78.19 1.01.51l.79 1.09h5.66c.69 0 1.25.56 1.25 1.25v7.15c0 .69-.56 1.25-1.25 1.25H2.75c-.69 0-1.25-.56-1.25-1.25V3.5Z"
                    /></svg
                  >
                </button>
              {:else}
                <span class="value" title={row.title}>
                  {#if row.href}
                    <a href={row.href} target="_blank" rel="noreferrer">{row.value}</a>
                  {:else}
                    {row.value}
                  {/if}
                </span>
                {#if row.tag}<span class="tag">{row.tag}</span>{/if}
                {#if row.action === "check"}
                  <button type="button" class="link" onclick={oncheckupdates}>Check now</button>
                {/if}
              {/if}
            </dd>
          {/each}
        </dl>
      </section>
    {/each}
  </div>

  {#snippet footer()}
    <button type="button" class="btn" onclick={copy}>
      {copied ? "Copied ✓" : "Copy Diagnostics"}
    </button>
    <button type="button" class="btn primary" bind:this={closer} onclick={onclose}>Close</button>
  {/snippet}
</Dialog>

<style>
  .about {
    display: flex;
    flex-direction: column;
    gap: var(--sp-6);
  }

  header {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-5);
  }

  header img {
    flex: 0 0 64px;
    border-radius: var(--r-md);
  }

  header p {
    margin: 0;
  }

  .name {
    font-size: 18px;
    font-weight: 600;
    line-height: 24px;
  }

  .muted {
    margin-top: var(--sp-1);
    color: var(--text-secondary);
    font-size: var(--fs-dense);
  }

  h3 {
    margin: 0 0 var(--sp-3);
    color: var(--text-secondary);
    font-size: var(--fs-header);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  dl {
    display: grid;
    grid-template-columns: 88px minmax(0, 1fr);
    align-items: baseline;
    gap: var(--sp-2) var(--sp-4);
    margin: 0;
    font-size: var(--fs-dense);
    line-height: var(--lh-dense);
  }

  dt {
    color: var(--text-secondary);
  }

  dd {
    display: flex;
    align-items: baseline;
    gap: var(--sp-3);
    min-width: 0;
    margin: 0;
  }

  .value {
    min-width: 0;
    overflow: hidden;
    color: var(--text-code);
    font-family: var(--font-mono);
    font-size: var(--fs-dense);
    white-space: nowrap;
    text-overflow: ellipsis;
    user-select: text;
    cursor: text;
  }

  .path {
    flex: 1 1 auto;
    color: var(--link);
    cursor: pointer;
  }

  .path:hover {
    text-decoration: underline;
  }

  .tag {
    padding: 0 var(--sp-2);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    color: var(--status-modify);
    font-size: var(--fs-header);
  }

  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--link);
    font: inherit;
    cursor: pointer;
  }

  .link:hover {
    text-decoration: underline;
  }

  .folder {
    display: inline-flex;
    flex: none;
    align-self: center;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-secondary);
    cursor: pointer;
  }

  .folder:hover {
    color: var(--text-primary);
  }

  .folder svg {
    width: 14px;
    height: 14px;
  }
</style>
