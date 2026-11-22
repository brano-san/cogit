import { emitTo, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  listRepositories, onMenuCommand, popupContextMenu, previewUserCommand, repositoryTools, runUserCommand, trustRepositoryTools,
  type ContextItem, type RepoId, type ToolContext,
} from "$lib/ipc";
import { desktopBlock } from "$lib/ipc/repo-rows";
import { effective, prettyKeys } from "$lib/keymap";
import { listenUntilUndone } from "$lib/settings-sync";
import {
  knownUserCommands, repositoryToolShortcuts, toolMenuId, toolRequest, toolShortcuts, toolsSubmenu, withTools, type ToolPlace, type UserCommand,
} from "$lib/user-commands";
import { confirmation } from "$stores/confirm.svelte";
import { errors } from "$stores/errors.svelte";
import { notices } from "$stores/notices.svelte";
import { output } from "$stores/output.svelte";
import { settings } from "$stores/settings.svelte";

interface Target {
  repo: RepoId;
  context: ToolContext;
  commands: Map<string, UserCommand>;
}

type ToolFile = Awaited<ReturnType<typeof repositoryTools>>;
const CHILD_TOOL = "cogit-line-tool";

interface LineToolSelection {
  id: string;
  source: UserCommand["source"];
  repo: RepoId;
  context: ToolContext;
}

/** User commands (F-954): the Tools submenus remember what they were opened on, and a
    command picked from one of them or from the palette runs here. */
class UserToolsStore {
  #target: Target | null = null;
  #serial = 0;
  #loading = new Map<RepoId, Promise<ToolFile>>();
  #refresh: (repo: RepoId) => Promise<void> = async () => {};
  #message: (repo: RepoId, stdout: string, replace: boolean) => Promise<void> = async () => {};
  #launch = (command: UserCommand, repo: RepoId, context: ToolContext): void => { void this.run(command, repo, context); };
  files = $state.raw<ReadonlyMap<RepoId, ToolFile>>(new Map());

  /** What Refresh after does: App re-reads the repository and the panels. */
  onRefresh(refresh: (repo: RepoId) => Promise<void>): void {
    this.#refresh = refresh;
  }

  onMessage(write: (repo: RepoId, stdout: string, replace: boolean) => Promise<void>): void {
    this.#message = write;
  }

  load(repo: RepoId): Promise<ToolFile> {
    const loading = this.#loading.get(repo);
    if (loading) return loading;
    const pending = repositoryTools(repo).catch((err) => {
      errors.report(err, "Could not read repository commands", String(repo));
      return null;
    }).then((file) => {
      const before = this.files.get(repo);
      if (before?.hash !== file?.hash || before?.trusted !== file?.trusted) this.files = new Map(this.files).set(repo, file);
      return file;
    }).finally(() => this.#loading.delete(repo));
    this.#loading.set(repo, pending);
    return pending;
  }

  commandsIn(file: ToolFile | undefined): UserCommand[] {
    if (!file) return [];
    return (knownUserCommands(file.commands) ?? []).map((command, index) => ({
      ...command, source: { index, hash: file.hash },
    }));
  }

  async lineMenu(repo: RepoId, context: ToolContext, event: MouseEvent): Promise<void> {
    event.preventDefault();
    try {
      const opened = (await listRepositories()).find((entry) => entry.repo === repo);
      if (!opened) return;
      const tools = await this.menus("file", repo, context, await desktopBlock(opened.root));
      if (tools.some(Boolean)) await popupContextMenu(withTools([], ...tools), event.clientX, event.clientY);
    } catch (err) {
      errors.report(err, "Could not open the line command menu");
    }
  }

  relayLineMenus(): () => void {
    this.#launch = (command, repo, context) => {
      const selection: LineToolSelection = { id: command.id, source: command.source, repo, context };
      void emitTo("main", CHILD_TOOL, selection).catch((err) => errors.report(err, "Could not run the line command"));
    };
    void settings.loadBindings();
    return listenUntilUndone((emit) => onMenuCommand((id) => emit({ payload: id })), (id: string) => this.runMenu(id));
  }

  receiveLineMenus(): () => void {
    return listenUntilUndone<LineToolSelection>((handler) => listen(CHILD_TOOL, handler), (selection) => {
      void this.#runLineSelection(selection).catch((err) => errors.report(err, "Could not run the line command"));
    });
  }

  async #runLineSelection({ id, source, repo, context }: LineToolSelection): Promise<void> {
    const file = source ? await this.load(repo) : null;
    const command = source
      ? this.commandsIn(file).find((command) => command.source?.hash === source.hash && command.source?.index === source.index)
      : settings.current.userCommands.find((command) => command.id === id);
    if (!command) {
      notices.inform("Command changed", "Open the line menu again to review the available commands.");
      return;
    }
    if (command.confirm || (source && !file?.trusted) || command.output === "message" || command.output === "panel") {
      await getCurrentWindow().setFocus();
    }
    await this.run(command, repo, context);
  }

  async menus(place: ToolPlace, repo: RepoId | null, context: ToolContext, blocked: string | null = null): Promise<(ContextItem | null)[]> {
    const file = repo === null ? null : await this.load(repo);
    const personal = settings.current.userCommands;
    const shared = this.commandsIn(file);
    const builtin = effective(settings.bindings, settings.keymap);
    const shortcuts = toolShortcuts(personal, builtin);
    const repositoryKeys = file?.trusted ? repositoryToolShortcuts(shared, personal, builtin) : {};
    const menus = [
      toolsSubmenu(personal, place, context, blocked ?? (repo === null ? "closed" : null), shortcuts),
      toolsSubmenu(shared, place, context, blocked, repositoryKeys, true, file?.trusted),
    ];
    if (repo === null || blocked) return menus;
    const byId = new Map([...personal, ...shared].map((command) => [toolMenuId(command), command]));
    const commands = new Map<string, UserCommand>();
    const prefix = `user-tool:${getCurrentWindow().label}:${++this.#serial}:`;
    for (const menu of menus) {
      for (const row of menu?.children ?? []) {
        const command = byId.get(row.id);
        if (!command) continue;
        row.id = `${prefix}${commands.size}`;
        commands.set(row.id, command);
      }
    }
    this.#target = { repo, context, commands };
    return menus;
  }

  /** True when `id` was a Tools row and has been taken care of. */
  runMenu(id: string): boolean {
    if (!id.startsWith("user-tool:")) return false;
    const target = this.#target;
    const command = target?.commands.get(id);
    if (command && target) {
      this.#target = null;
      this.#launch(command, target.repo, target.context);
    }
    return true;
  }

  async run(command: UserCommand, repo: RepoId, context: ToolContext): Promise<void> {
    try {
      if (command.source && !await this.#review(repo, command)) return;
      const request = toolRequest(command, context);
      if (command.confirm) {
        const preview = await previewUserCommand(repo, request);
        if (!await confirmation.ask({
          title: command.name,
          message: `Run ${command.name}?`,
          confirm: "Run",
          fields: [["Command", preview.line], ["Folder", preview.cwd]],
        })) return;
        request.confirmation = preview.confirmation;
      }
      try {
        const run = await runUserCommand(repo, request);
        if (!command.detached && run.exitCode === 0) await this.#showOutput(command, repo, run.record, run.presentation);
      } finally {
        if (command.refresh && !command.detached) {
          await this.#refresh(repo).catch((err) => errors.report(err, "Could not refresh after running the command"));
        }
      }
    } catch (err) {
      errors.report(err, `Could not run ${command.name}`);
    }
  }

  async #review(repo: RepoId, command: UserCommand): Promise<boolean> {
    const file = await this.load(repo);
    if (!file || file.hash !== command.source?.hash) {
      notices.inform("Repository commands changed", "Open the menu again to review the current command file.");
      return false;
    }
    if (file.trusted) return true;
    const accepted = await confirmation.ask({
      title: "Trust Repository Commands",
      message: "Commands in this file run on your computer with your permissions. Review all commands before trusting this version of the file.",
      fields: [["File", ".cogit/commands.json"], ...this.commandsIn(file).map((tool): [string, string] => [
        tool.name,
        `${tool.program} ${tool.args}\nWorking directory: ${tool.cwd || "${repo}"}${tool.shortcut ? `\nShortcut: ${prettyKeys(tool.shortcut)}` : ""}`,
      ])],
      confirm: "Trust and Run",
      warning: true,
    });
    if (!accepted) return false;
    await trustRepositoryTools(repo, file.hash);
    await this.load(repo);
    return true;
  }

  async #showOutput(command: UserCommand, repo: RepoId, record: number, presentation: string | null): Promise<void> {
    if (command.output === "silent") return;
    if (command.output === "panel") {
      await output.openRecord(record);
      return;
    }
    if (presentation === null) return;
    if (command.output === "message") await this.#message(repo, presentation, command.replaceMessage);
    else notices.inform(command.name, presentation || "The command completed without output.");
  }
}

export const userTools = new UserToolsStore();
