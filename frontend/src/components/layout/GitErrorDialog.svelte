<script lang="ts">
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import type { CogitError } from "$lib/ipc";

  interface Props {
    error: CogitError;
    ondismiss: () => void;
  }

  let { error, ondismiss }: Props = $props();
  const copied = $state({ done: false });

  async function copy() {
    await writeText(error.message);
    copied.done = true;
    setTimeout(() => (copied.done = false), 1500);
  }
</script>

<!-- Refusals Cogit made on its own: a branch name it will not accept, a file it cannot
     read. Anything with git output behind it goes to the output window instead, which
     can hold a hook's thousand lines (doc/12-risks.md, R-89). -->
<div class="dialog" role="alertdialog" aria-label="Cogit could not do that">
  <header>
    <span class="title">Cogit stopped</span>
    <span class="grow"></span>
    <button type="button" onclick={copy}>{copied.done ? "Copied" : "Copy"}</button>
    <button type="button" onclick={ondismiss} title="Dismiss">✕</button>
  </header>

  <p class="message">{error.message}</p>
</div>

<style>
  .dialog {
    position: absolute;
    right: var(--sp-5);
    bottom: var(--sp-5);
    z-index: 10;
    display: flex;
    flex-direction: column;
    max-width: min(560px, 60vw);
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
    margin-bottom: var(--sp-3);
  }

  .title {
    font-weight: 600;
    color: var(--status-delete);
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

  .message {
    margin: 0;
    color: var(--text-primary);
    user-select: text;
    overflow-wrap: anywhere;
  }
</style>
