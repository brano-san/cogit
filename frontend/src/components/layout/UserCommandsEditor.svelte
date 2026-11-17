<script lang="ts">
  import Icon from "$components/common/Icon.svelte";
  import Checkbox from "$components/common/Checkbox.svelte";
  import Radio from "$components/common/Radio.svelte";
  import { ON_WINDOWS } from "$lib/platform";
  import {
    commandProblem,
    MAX_USER_COMMANDS,
    newUserCommand,
    TOOL_PLACES,
    type ToolPlace,
    type UserCommand,
  } from "$lib/user-commands";

  /** Preferences ▸ Tools (F-954): the list on top, the chosen command's fields under it.
      Every edit is applied and saved at once, like the rest of Preferences (R-122). */
  interface Props {
    commands: readonly UserCommand[];
    onchange: (next: UserCommand[]) => void;
  }

  let { commands, onchange }: Props = $props();

  let picked = $state<string | null>(null);
  const index = $derived.by(() => {
    const at = commands.findIndex((command) => command.id === picked);
    return at >= 0 ? at : commands.length > 0 ? 0 : -1;
  });
  const current = $derived(index >= 0 ? commands[index] : undefined);
  const problem = $derived(current ? commandProblem(current) : null);

  const PLACE_LABELS: Record<ToolPlace, string> = {
    repository: "Repositories menu",
    file: "Files menu",
    commit: "Graph commit menu",
  };

  function edit(patch: Partial<UserCommand>) {
    if (!current) return;
    onchange(commands.map((command, at) => (at === index ? { ...command, ...patch } : command)));
  }

  function add() {
    const fresh = newUserCommand(commands);
    picked = fresh.id;
    onchange([...commands, fresh]);
  }

  function remove() {
    if (!current) return;
    const next = commands.filter((_, at) => at !== index);
    picked = next[Math.min(index, next.length - 1)]?.id ?? null;
    onchange(next);
  }

  function move(delta: -1 | 1) {
    const to = index + delta;
    if (!current || to < 0 || to >= commands.length) return;
    const next = [...commands];
    next.splice(index, 1);
    next.splice(to, 0, current);
    onchange(next);
  }

  function togglePlace(place: ToolPlace, on: boolean) {
    if (!current) return;
    const places = TOOL_PLACES.filter((each) => (each === place ? on : current.places.includes(each)));
    edit({ places });
  }

  async function browse() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const chosen = await open({
      title: "Select Program",
      multiple: false,
      directory: false,
      filters: ON_WINDOWS ? [{ name: "Programs", extensions: ["exe", "cmd", "bat", "com"] }] : undefined,
    });
    if (typeof chosen === "string") edit({ program: chosen });
  }
</script>

<div class="editor">
  <div class="top">
    <ul class="list" role="listbox" aria-label="User commands">
      {#each commands as command, at (command.id)}
        <li>
          <button
            type="button"
            role="option"
            aria-selected={at === index}
            class:selected={at === index}
            class:bad={commandProblem(command) !== null}
            onclick={() => (picked = command.id)}>{command.name || "(no name)"}</button
          >
        </li>
      {:else}
        <li class="empty">No commands yet</li>
      {/each}
    </ul>
    <div class="moves">
      <button type="button" class="btn" disabled={commands.length >= MAX_USER_COMMANDS} onclick={add}>Add</button>
      <button type="button" class="btn" disabled={!current} onclick={remove}>Remove</button>
      <button type="button" class="btn" disabled={!current || index === 0} onclick={() => move(-1)}>Move Up</button>
      <button type="button" class="btn" disabled={!current || index === commands.length - 1} onclick={() => move(1)}
        >Move Down</button
      >
    </div>
  </div>

  {#if current}
    {#key current.id}
      <div class="form">
        <label class="field">
          <span>Name</span>
          <input type="text" class="text" value={current.name} oninput={(e) => edit({ name: e.currentTarget.value })} />
        </label>
        <div class="field">
          <span id="tool-program">Program</span>
          <div class="path">
            <input
              type="text"
              class="text"
              aria-labelledby="tool-program"
              placeholder={ON_WINDOWS ? "C:\\Program Files\\Tool\\tool.exe" : "/usr/bin/tool"}
              value={current.program}
              oninput={(e) => edit({ program: e.currentTarget.value })}
            />
            <button type="button" class="btn browse" title="Browse…" aria-label="Browse for the program" onclick={browse}
              >...</button
            >
          </div>
        </div>
        <label class="field">
          <span>Arguments</span>
          <input
            type="text"
            class="text mono"
            placeholder={'log --follow -- "${relfile}"'}
            value={current.args}
            oninput={(e) => edit({ args: e.currentTarget.value })}
          />
        </label>
        <label class="field">
          <span>Working directory</span>
          <input
            type="text"
            class="text mono"
            placeholder={"${repo}"}
            value={current.cwd}
            oninput={(e) => edit({ cwd: e.currentTarget.value })}
          />
        </label>
        <div class="field" role="group" aria-label="Show in">
          <span>Show in</span>
          <div class="options">
            {#each TOOL_PLACES as place (place)}
              <Checkbox
                checked={current.places.includes(place)}
                label={PLACE_LABELS[place]}
                onchange={(on) => togglePlace(place, on)}
              />
            {/each}
          </div>
        </div>
        <div class="field" role="radiogroup" aria-label="When it runs">
          <span>When it runs</span>
          <div class="options">
            <Radio
              name="tool-mode"
              checked={!current.detached}
              label="Show output: wait for it, its output goes to Output; Stop in the footer ends it"
              onchange={() => edit({ detached: false })}
            />
            <Radio
              name="tool-mode"
              checked={current.detached}
              label="Run detached: start it and leave it running (a GUI tool)"
              onchange={() => edit({ detached: true, refresh: false })}
            />
          </div>
        </div>
        <div class="field">
          <span></span>
          <div class="options">
            <Checkbox
              checked={current.confirm}
              label="Ask for confirmation"
              onchange={(on) => edit({ confirm: on })}
            />
            <Checkbox
              checked={current.refresh && !current.detached}
              disabled={current.detached}
              label="Refresh the repository when it ends"
              onchange={(on) => edit({ refresh: on })}
            />
          </div>
        </div>
        {#if problem}
          <p class="verdict" role="alert"><Icon name="error" /> {problem}</p>
        {/if}
      </div>
    {/key}
  {/if}
</div>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    margin-bottom: var(--sp-3);
    font-size: var(--fs-dense);
  }

  .top {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: var(--sp-4);
  }

  .list {
    height: calc(var(--h-row) * 5);
    margin: 0;
    padding: var(--sp-1) 0;
    overflow-y: auto;
    list-style: none;
    background: var(--surface-input);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
  }

  .list button {
    display: block;
    width: 100%;
    padding: var(--sp-1) var(--sp-4);
    background: none;
    border: 0;
    color: var(--text-primary);
    font: inherit;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: default;
  }

  .list button:hover {
    background: var(--state-hover);
  }

  .list button.selected {
    background: var(--state-selected);
  }

  .list button.bad {
    color: var(--status-delete);
  }

  .empty {
    padding: var(--sp-2) var(--sp-4);
    color: var(--text-secondary);
  }

  .moves {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }

  .field {
    display: grid;
    grid-template-columns: 160px minmax(0, 1fr);
    align-items: center;
    gap: var(--sp-4);
    min-height: var(--h-row);
  }

  .field[role="group"],
  .field[role="radiogroup"],
  .field:has(.options) {
    align-items: start;
  }

  .options {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
  }

  .text {
    min-width: 0;
    height: var(--h-input);
    padding: 0 var(--sp-3);
    background: var(--surface-input);
    color: var(--text-primary);
    border: 1px solid var(--field-border);
    border-radius: var(--r-sm);
  }

  .path {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
  }

  .path .text {
    flex: 1 1 0;
  }

  .browse {
    flex: 0 0 auto;
  }

  .verdict {
    margin: 0;
    color: var(--status-delete);
    font-size: var(--fs-header);
    overflow-wrap: anywhere;
  }

  @container (max-width: 440px) {
    .field,
    .top {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
