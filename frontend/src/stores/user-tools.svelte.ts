import { runUserCommand, type ContextItem, type RepoId, type ToolContext } from "$lib/ipc";
import { toolOf, toolRequest, toolsSubmenu, type ToolPlace, type UserCommand } from "$lib/user-commands";
import { confirmation } from "$stores/confirm.svelte";
import { errors } from "$stores/errors.svelte";
import { output } from "$stores/output.svelte";
import { settings } from "$stores/settings.svelte";

interface Target {
  repo: RepoId;
  context: ToolContext;
}

/** User commands (F-954): the Tools submenus remember what they were opened on, and a
    command picked from one of them or from the palette runs here. */
class UserToolsStore {
  #target: Target | null = null;
  #refresh: (repo: RepoId) => Promise<void> = async () => {};

  /** What Refresh after does: App re-reads the repository and the panels. */
  onRefresh(refresh: (repo: RepoId) => Promise<void>): void {
    this.#refresh = refresh;
  }

  /** The Tools submenu for a menu about to open on `context`; `repo` null is a closed row. */
  submenu(place: ToolPlace, repo: RepoId | null, context: ToolContext): ContextItem | null {
    this.#target = repo === null ? null : { repo, context };
    return toolsSubmenu(settings.current.userCommands, place, context, repo === null ? "closed" : null);
  }

  /** True when `id` was a Tools row and has been taken care of. */
  runMenu(id: string): boolean {
    const tool = toolOf(id);
    if (tool === null) return false;
    const command = settings.current.userCommands.find((each) => each.id === tool);
    const target = this.#target;
    if (command && target) void this.run(command, target.repo, target.context);
    return true;
  }

  async run(command: UserCommand, repo: RepoId, context: ToolContext): Promise<void> {
    if (command.confirm) {
      const yes = await confirmation.ask({
        title: command.name,
        message: `Run ${command.name}?`,
        confirm: "Run",
        fields: [
          ["Program", command.program],
          ["Arguments", command.args],
          ["Folder", command.cwd || "${repo}"],
        ],
      });
      if (!yes) return;
    }
    try {
      const run = await runUserCommand(repo, toolRequest(command, context));
      // A failure raises its own notice from the journal; a success shows what it printed.
      if (!command.detached && run.exitCode === 0) await output.openRecord(run.record);
    } catch (err) {
      errors.report(err, `Could not run ${command.name}`);
      return;
    }
    if (command.refresh && !command.detached) await this.#refresh(repo);
  }
}

export const userTools = new UserToolsStore();
