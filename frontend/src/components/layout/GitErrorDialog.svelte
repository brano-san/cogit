<script lang="ts">
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { splitLinks } from "$lib/links";
  import type { CogitError } from "$lib/ipc";

  interface Props {
    error: CogitError;
    ondismiss: () => void;
  }

  let { error, ondismiss }: Props = $props();

  const details = $derived(error.detail.kind === "command" ? error.detail.data : null);
  const copied = $state({ done: false });

  function plainText(): string {
    if (!details) return error.message;
    return [
      `$ ${details.command}`,
      `exit code: ${details.exitCode ?? "unknown"}`,
      "",
      "--- stdout ---",
      details.stdout,
      "--- stderr ---",
      details.stderr,
    ].join("\n");
  }

  function streams(data: { stdout: string; stderr: string }) {
    return [
      { label: "stderr", stream: data.stderr },
      { label: "stdout", stream: data.stdout },
    ];
  }

  async function copy() {
    await writeText(plainText());
    copied.done = true;
    setTimeout(() => (copied.done = false), 1500);
  }
</script>

<div class="dialog" role="dialog" aria-label="Git error">
  <header>
    <span class="title">Git failed</span>
    {#if details}<span class="code mono">exit {details.exitCode ?? "?"}</span>{/if}
    <span class="grow"></span>
    <button type="button" onclick={copy}>{copied.done ? "Copied" : "Copy Output"}</button>
    <button type="button" onclick={ondismiss} title="Dismiss">✕</button>
  </header>

  {#if details}
    <p class="command mono">$ {details.command}</p>
    {#each streams(details) as pane (pane.label)}
      {#if pane.stream.trim() !== ""}
        <p class="label">{pane.label}</p>
        <pre class="stream mono">{#each splitLinks(pane.stream) as part, i (i)}{#if part.href}<a
                href={part.href}
                onclick={(event) => {
                  event.preventDefault();
                  void openUrl(part.href ?? "");
                }}>{part.text}</a
              >{:else}{part.text}{/if}{/each}</pre>
      {/if}
    {/each}
  {:else}
    <p class="command">{error.message}</p>
  {/if}
</div>

<style>
  .dialog {
    position: absolute;
    right: var(--sp-5);
    bottom: var(--sp-5);
    z-index: 10;
    display: flex;
    flex-direction: column;
    max-width: min(720px, 70vw);
    max-height: 50vh;
    overflow: auto;
    padding: var(--sp-4) var(--sp-5) var(--sp-5);
    background: var(--surface-panel);
    border: 1px solid var(--status-delete);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-dialog);
    font-size: var(--fs-dense);
  }

  header {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    margin-bottom: var(--sp-4);
  }

  .title {
    font-weight: 600;
    color: var(--status-delete);
  }

  .code {
    color: var(--text-secondary);
    font-size: 11px;
  }

  .grow {
    flex: 1 1 auto;
  }

  button {
    height: var(--h-button-sm);
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
    font-size: var(--fs-dense);
    cursor: default;
  }

  button:hover {
    border-color: var(--status-ref);
  }

  .command {
    margin: 0 0 var(--sp-4);
    color: var(--text-secondary);
    user-select: text;
  }

  .label {
    margin: 0 0 var(--sp-2, 3px);
    color: var(--text-secondary);
    font-size: var(--fs-header);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  /* Raw Git output is never reformatted or truncated (INV-05). */
  .stream {
    margin: 0 0 var(--sp-4);
    padding: var(--sp-4);
    background: var(--surface-input);
    border-radius: var(--r-sm);
    font-size: var(--fs-code);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: text;
  }

  a {
    color: var(--status-ref);
  }
</style>
