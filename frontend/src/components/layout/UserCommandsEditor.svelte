<script lang="ts">
  import Icon from "$components/common/Icon.svelte";
  import Checkbox from "$components/common/Checkbox.svelte";
  import Radio from "$components/common/Radio.svelte";
  import { captureKeys, exportToolCommands, importToolCommands } from "$lib/ipc";
  import { prettyKeys, recordKeys, type Keymap } from "$lib/keymap";
  import { ON_MAC, ON_WINDOWS } from "$lib/platform";
  import { errors } from "$stores/errors.svelte";
  import {
    commandProblem,
    MAX_USER_COMMANDS,
    newUserCommand,
    knownUserCommands,
    mergeToolCommands,
    OUTPUT_MODES,
    PLACEHOLDERS,
    toolShortcutProblem,
    TOOL_PLACES,
    type ToolOutput,
    type ToolPlace,
    type UserCommand,
  } from "$lib/user-commands";

  /** Preferences ▸ Tools (F-954): the list on top, the chosen command's fields under it.
      Every edit is applied and saved at once, like the rest of Preferences (R-122). */
  interface Props {
    commands: readonly UserCommand[];
    builtins: Keymap;
    bindingLabels: Record<string, string>;
    onchange: (next: UserCommand[]) => void;
  }

  let { commands, builtins, bindingLabels, onchange }: Props = $props();

  let picked = $state<string | null>(null);
  const index = $derived.by(() => {
    const at = commands.findIndex((command) => command.id === picked);
    return at >= 0 ? at : commands.length > 0 ? 0 : -1;
  });
  const current = $derived(index >= 0 ? commands[index] : undefined);
  const problem = $derived(current ? commandProblem(current) : null);
  const shortcutProblem = $derived(current ? toolShortcutProblem(current, commands, builtins, ON_MAC, bindingLabels) : null);
  let marked = $state<string[]>([]);
  const checked = $derived(commands.filter((command) => marked.includes(command.id)));
  const selected = $derived(checked.length ? checked : current ? [current] : []);
  let transferring = $state(false);
  let capturing = $state(false);
  let refusal = $state<string | null>(null);

  $effect(() => {
    if (!capturing) return;
    void captureKeys(true);
    return () => void captureKeys(false);
  });

  const OUTPUT_LABELS: Record<ToolOutput, string> = {
    panel: "Show in Output",
    notification: "Show the last lines in a notification",
    message: "Insert into the commit message",
    silent: "Silent",
  };

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
    marked = marked.filter((id) => id !== current.id);
    picked = next[Math.min(index, next.length - 1)]?.id ?? null;
    onchange(next);
  }

  function capture(event: KeyboardEvent) {
    if (!capturing) return;
    if (event.key === "Tab" && !event.ctrlKey && !event.altKey && !event.metaKey) {
      capturing = false;
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape") {
      capturing = false;
      return;
    }
    const recorded = recordKeys(event, ON_MAC);
    if (!recorded) return;
    if ("refused" in recorded) {
      refusal = recorded.refused;
      return;
    }
    capturing = false;
    refusal = null;
    edit({ shortcut: recorded.keys });
  }

  async function transfer(kind: "import" | "export") {
    transferring = true;
    const targets = selected;
    try {
      const { open, save } = await import("@tauri-apps/plugin-dialog");
      const filters = [{ name: "JSON command files", extensions: ["json"] }];
      if (kind === "export") {
        const path = await save({ title: "Export Commands", defaultPath: "commands.json", filters });
        if (path) await exportToolCommands(path, targets.map(({ source: _, ...command }) => command));
      } else {
        const path = await open({ title: "Import Commands", multiple: false, filters });
        if (typeof path !== "string") return;
        const imported = knownUserCommands(await importToolCommands(path)) ?? [];
        if (commands.length + imported.length > MAX_USER_COMMANDS) {
          errors.message("You can save up to 50 commands. Remove some commands and try again.", "Could not import commands");
          return;
        }
        const merged = mergeToolCommands(commands, imported);
        picked = merged[commands.length]?.id ?? picked;
        onchange(merged);
      }
    } catch (err) {
      errors.report(err, kind === "import" ? "Could not import commands" : "Could not export commands");
    } finally {
      transferring = false;
    }
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
    <ul class="list" aria-label="User commands">
      {#each commands as command, at (command.id)}
        <li class="command-row" class:selected={at === index}>
          <Checkbox
            checked={marked.includes(command.id)}
            ariaLabel={`Select ${command.name} for export`}
            onchange={(on) => (marked = on ? [...marked, command.id] : marked.filter((id) => id !== command.id))}
          />
          <button
            type="button"
            aria-current={at === index ? "true" : undefined}
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
      <button type="button" class="btn" disabled={transferring || commands.length >= MAX_USER_COMMANDS} onclick={() => void transfer("import")}>Import…</button>
      <button type="button" class="btn" disabled={transferring || !selected.length || selected.some((command) => commandProblem(command) !== null)} onclick={() => void transfer("export")}>Export Selected…</button>
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
        <div class="field">
          <span></span>
          <p class="help mono">{PLACEHOLDERS.map((name) => "$" + "{" + name + "}").join(", ")}</p>
        </div>
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
        <label class="field">
          <span>Applies to</span>
          <input type="text" class="text mono" placeholder="*.png; docs/**/*.md" value={current.patterns}
            disabled={!current.places.includes("file")} oninput={(e) => edit({ patterns: e.currentTarget.value })} />
        </label>
        <div class="field" role="group" aria-label="Shortcut">
          <span>Shortcut</span>
          <div class="path">
            <button type="button" class="btn" onclick={() => { capturing = !capturing; refusal = null; }}
              onkeydown={capture} onblur={() => (capturing = false)}>
              {capturing ? "Press keys…" : prettyKeys(current.shortcut)}
            </button>
            <button type="button" class="btn" disabled={!current.shortcut} onclick={() => { refusal = null; edit({ shortcut: "" }); }}>Remove Key</button>
          </div>
        </div>
        {#if (capturing && refusal) || shortcutProblem}
          <p class="verdict" role="status">{capturing && refusal ? refusal : shortcutProblem}</p>
        {/if}
        <div class="field" role="radiogroup" aria-label="When it runs">
          <span>When it runs</span>
          <div class="options">
            <Radio
              name="tool-mode"
              checked={!current.detached}
              label="Wait for the command to finish"
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
        {#if !current.detached}
          <div class="field" role="radiogroup" aria-label="Output">
            <span>Output</span>
            <div class="options">
              {#each OUTPUT_MODES as mode (mode)}
                <Radio name="tool-output" checked={current.output === mode} label={OUTPUT_LABELS[mode]} onchange={() => edit({ output: mode })} />
              {/each}
              {#if current.output === "message"}
                <Checkbox checked={current.replaceMessage} label="Replace the commit message" onchange={(on) => edit({ replaceMessage: on })} />
              {/if}
            </div>
          </div>
        {/if}
        <div class="field">
          <span></span>
          <div class="options">
            <Checkbox
              checked={current.confirm}
              label="Ask before running"
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

  .command-row {
    display: flex;
    align-items: center;
    padding-left: var(--sp-4);
  }

  .command-row.selected {
    background: var(--state-selected);
  }

  .help {
    margin: 0;
    color: var(--text-secondary);
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
