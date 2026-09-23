import { openSubmodule, type RepoId } from "$lib/ipc";
import {
  addSubmodule,
  lfsOp,
  lfsVersion,
  submoduleOp,
  subtreeOp,
  subtreePrefixes,
  type LfsOp,
} from "$lib/ipc/remote-ops";
import {
  LFS_DOWNLOAD,
  lfsMissingDialog,
  lfsPruneDialog,
  lfsTrackDialog,
  lfsTrackRequest,
  submoduleAddDialog,
  submoduleDialog,
  subtreeDialog,
  subtreeRequest,
  text,
  type DialogSpec,
  type Values,
} from "$lib/remote-dialogs";
import {
  submoduleScope,
  trackSuggestion,
  type LfsAction,
  type RemoteMenuActions,
  type SubmoduleAction,
  type SubtreeAction,
} from "$lib/remote-menu";
import { errors } from "$stores/errors.svelte";
import { network } from "$stores/network.svelte";
import { repository } from "$stores/repository.svelte";
import { submodules } from "$stores/submodules.svelte";

/** What the App hands over: the parts of its state this store cannot read itself. */
export interface RemoteHost {
  /** Ticked in the Files panel, or else the one open in the Diff panel. */
  files: () => readonly string[];
  /** The refresh every change to the repository ends with. */
  changed: () => Promise<void>;
  synchronize: () => void;
  repoSettings: () => void;
}

interface Open {
  spec: DialogSpec;
  /** Resolves once the dialog may close; the work itself is reported, not thrown. */
  submit: (values: Values) => Promise<void>;
  /** A link the dialog shows under its text. */
  link?: string;
}

class RemoteOpsStore {
  dialog = $state.raw<Open | null>(null);
  /** `undefined` until asked, `null` when git has no `lfs` command. */
  lfs = $state<string | null | undefined>(undefined);

  async detectLfs(): Promise<void> {
    this.lfs = await lfsVersion().catch(() => null);
  }

  close(): void {
    this.dialog = null;
  }

  async submit(values: Values): Promise<void> {
    const open = this.dialog;
    if (!open) return;
    this.dialog = null;
    await open.submit(values);
  }

  actions(host: RemoteHost): RemoteMenuActions {
    return {
      synchronize: host.synchronize,
      repoSettings: host.repoSettings,
      submodule: (action) => void this.#submodule(action, host),
      subtree: (action) => void this.#subtree(action, host),
      lfs: (action) => void this.#lfs(action, host),
    };
  }

  /** A failure goes to the notifications with git's own output. */
  async #perform(
    host: RemoteHost,
    title: string,
    work: () => Promise<unknown>,
    refresh = true,
  ): Promise<void> {
    try {
      await work();
    } catch (err) {
      errors.report(err, title);
    }
    if (refresh) await host.changed();
  }

  /** The repository that holds the submodules in `parent`: the top one, or an opened one. */
  async #owner(parent: string): Promise<RepoId | null> {
    const top = submodules.owner;
    if (parent === "" || top === null) return top;
    try {
      return (await openSubmodule(top, parent)).repo;
    } catch (err) {
      errors.report(err, "Could not open the submodule");
      return null;
    }
  }

  async #submodule(action: SubmoduleAction, host: RemoteHost): Promise<void> {
    const current = repository.current?.repo;
    if (action === "add") {
      if (!current) return;
      this.dialog = {
        spec: submoduleAddDialog(),
        submit: (values) =>
          this.#perform(host, "Could not add the submodule", () =>
            addSubmodule(current, text(values, "url"), text(values, "path"), text(values, "branch") || null),
          ),
      };
      return;
    }
    const scope = submoduleScope({
      children: submodules.children,
      open: submodules.open,
      selected: host.files(),
    });
    const owner = await this.#owner(scope.parent);
    if (!owner) return;
    if (action === "initialize" || action === "synchronize") {
      const paths = scope.path ? [scope.path] : [];
      await this.#perform(host, `Could not ${action} the submodules`, () => submoduleOp(owner, action, paths));
      return;
    }
    this.dialog = {
      spec: submoduleDialog(action, scope.choices, scope.path),
      submit: (values) =>
        this.#perform(host, `Could not ${action} the submodule`, () =>
          submoduleOp(owner, action, [text(values, "path")]),
        ),
    };
  }

  async #subtree(action: SubtreeAction, host: RemoteHost): Promise<void> {
    const repo = repository.current?.repo;
    if (!repo) return;
    const prefixes = action === "add" ? [] : await subtreePrefixes(repo).catch(() => []);
    this.dialog = {
      spec: subtreeDialog(action, { prefixes, remotes: network.remotes }),
      submit: (values) =>
        this.#perform(host, `Could not ${action} the subtree`, () => subtreeOp(repo, subtreeRequest(action, values))),
    };
  }

  async #lfs(action: LfsAction, host: RemoteHost): Promise<void> {
    const repo = repository.current?.repo;
    if (!repo) return;
    // Locks live on the server: nothing in the working tree changes, so nothing is reloaded.
    const run = (op: LfsOp) =>
      this.#perform(host, `Could not run git lfs ${op.kind}`, () => lfsOp(repo, op), op.kind !== "lock" && op.kind !== "unlock");
    switch (action) {
      case "install":
        if (this.lfs === null) {
          this.#explainMissing(false);
          return;
        }
        await run({ kind: "install" });
        return;
      case "track":
        this.dialog = {
          spec: lfsTrackDialog(trackSuggestion(host.files())),
          submit: (values) => run(lfsTrackRequest(values)),
        };
        return;
      case "lock":
      case "unlock":
        await run({ kind: action, paths: [...host.files()] });
        return;
      case "prune":
        this.dialog = { spec: lfsPruneDialog(), submit: () => run({ kind: "prune" }) };
        return;
    }
  }

  /** Check Again asks git once more; still nothing, and the dialog says so. */
  #explainMissing(again: boolean): void {
    this.dialog = {
      spec: lfsMissingDialog(again),
      link: LFS_DOWNLOAD,
      submit: async () => {
        await this.detectLfs();
        if (this.lfs === null) this.#explainMissing(true);
      },
    };
  }
}

export const remoteOps = new RemoteOpsStore();
