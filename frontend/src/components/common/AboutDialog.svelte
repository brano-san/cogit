<script lang="ts">
  import Dialog from "$components/common/Dialog.svelte";
  import { diagnosticsText, versionRows } from "$lib/diagnostics";
  import type { AppInfo } from "$lib/ipc";
  import icon from "../../../../src-tauri/icons/128x128.png";

  /** Every number anyone asks for in a bug report, and a button that copies the lot. */
  interface Props {
    info: AppInfo;
    onclose: () => void;
    oncopy: (text: string) => void;
    onreveallog: () => void;
  }

  let { info, onclose, oncopy, onreveallog }: Props = $props();

  const svelte = __SVELTE_VERSION__;
  const rows = $derived(versionRows(info, svelte));
  let copied = $state(false);

  function copy() {
    oncopy(diagnosticsText(info, svelte));
    copied = true;
  }
</script>

<Dialog title="About Cogit" {onclose} width="min(480px, 92vw)">
  <div class="about">
    <header>
      <img src={icon} alt="" width="64" height="64" />
      <div>
        <p class="name">Cogit</p>
        <p class="tagline">A Git client that says what it is doing.</p>
        <p class="licence">
          MIT licence ·
          <a href="https://github.com/brano-san/cogit" target="_blank" rel="noreferrer">
            github.com/brano-san/cogit
          </a>
        </p>
      </div>
    </header>

    <dl>
      {#each rows as [label, value] (label)}
        <dt>{label}</dt>
        <dd class="mono">{value}</dd>
      {/each}
      <dt>Log</dt>
      <dd class="mono path">
        <button type="button" class="link" onclick={onreveallog}>{info.logPath}</button>
      </dd>
    </dl>
  </div>

  {#snippet footer()}
    <button type="button" class="btn" onclick={copy}>
      {copied ? "Copied" : "Copy Diagnostics"}
    </button>
    <button type="button" class="btn primary" onclick={onclose}>Close</button>
  {/snippet}
</Dialog>

<style>
  .about {
    display: flex;
    flex-direction: column;
    gap: var(--s-4);
  }

  header {
    display: flex;
    align-items: flex-start;
    gap: var(--s-4);
  }

  header img {
    flex: 0 0 64px;
    border-radius: var(--r-md);
  }

  .name {
    margin: 0;
    font-size: 18px;
    font-weight: 600;
  }

  .tagline,
  .licence {
    margin: var(--s-1) 0 0;
    color: var(--text-secondary);
    font-size: 12px;
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--s-1) var(--s-4);
    margin: 0;
    font-size: 12px;
  }

  dt {
    color: var(--text-secondary);
  }

  dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  /* The log path is long and is the one row worth clicking. */
  .path {
    overflow-wrap: anywhere;
  }

  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .link:hover {
    text-decoration: underline;
  }
</style>
