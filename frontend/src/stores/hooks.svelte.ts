import {
  bypassLog,
  exportPreset,
  installPreset,
  listPresets,
  removePreset,
  CogitError,
  listHooks,
  readHook,
  runHook,
  setHookEnabled,
  useHooksPath,
  writeHook,
  type Bypass,
  type PresetStatus,
  type HookOverview,
  type HookRun,
  type RepoId,
} from "$lib/ipc";

class HooksStore {
  overview = $state.raw<HookOverview | null>(null);
  open = $state(false);
  editing = $state<string | null>(null);
  body = $state("");
  /** What was on disk when the editor opened, so "changed" is a comparison, not a guess. */
  saved = $state("");
  error = $state<CogitError | null>(null);
  /** What the failed action was, for the notice's title. */
  failure = $state<string | null>(null);
  lastRun = $state.raw<HookRun | null>(null);
  running = $state(false);
  bypasses = $state.raw<Bypass[]>([]);
  presets = $state.raw<PresetStatus[]>([]);
  showPresets = $state(false);

  async refresh(repo: RepoId): Promise<void> {
    try {
      this.overview = await listHooks(repo);
      this.bypasses = await bypassLog(repo);
      this.presets = await listPresets(repo);
    } catch (err) {
      this.report(err, "Could not read the hooks");
    }
  }

  async show(repo: RepoId): Promise<void> {
    this.open = true;
    await this.refresh(repo);
  }

  async edit(repo: RepoId, name: string): Promise<void> {
    this.editing = name;
    const present = this.overview?.hooks.find((hook) => hook.name === name);
    const body =
      present && present.state !== "missing"
        ? await readHook(repo, name).catch(() => "")
        : "#!/bin/sh\nset -e\n\n";
    // Another hook opened meanwhile; Save would write this body into that one.
    if (this.editing !== name) return;
    this.body = body;
    this.saved = body;
  }

  get dirty(): boolean {
    return this.editing !== null && this.body !== this.saved;
  }

  async save(repo: RepoId): Promise<void> {
    this.#clear();
    if (this.editing === null) return;
    try {
      await writeHook(repo, this.editing, this.body);
      this.editing = null;
      await this.refresh(repo);
    } catch (err) {
      this.report(err, "Could not save the hook");
    }
  }

  async toggle(repo: RepoId, name: string, enabled: boolean): Promise<void> {
    this.#clear();
    try {
      await setHookEnabled(repo, name, enabled);
      await this.refresh(repo);
    } catch (err) {
      this.report(err, "Could not switch the hook");
    }
  }

  async adopt(repo: RepoId, path: string): Promise<void> {
    this.#clear();
    try {
      await useHooksPath(repo, path);
      await this.refresh(repo);
    } catch (err) {
      this.report(err, "Could not use that hooks folder");
    }
  }

  /** Runs the hook alone, with no commit behind it. */
  async dryRun(repo: RepoId, name: string): Promise<void> {
    this.#clear();
    this.running = true;
    this.lastRun = null;
    try {
      this.lastRun = await runHook(repo, name);
    } catch (err) {
      this.report(err, "Could not run the hook");
    } finally {
      this.running = false;
    }
  }

  /** The hook as it stands becomes a preset; the id is derived from the name given. */
  async export(repo: RepoId, hook: string, name: string): Promise<void> {
    this.#clear();
    const id = name
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "");
    if (id === "") return;
    try {
      await exportPreset(repo, hook, id, name.trim(), `Saved from ${hook} in this repository`);
      await this.refresh(repo);
    } catch (err) {
      this.report(err, "Could not save the preset");
    }
  }

  async removeOwn(repo: RepoId, id: string): Promise<void> {
    this.#clear();
    try {
      await removePreset(id);
      await this.refresh(repo);
    } catch (err) {
      this.report(err, "Could not remove the preset");
    }
  }

  async install(repo: RepoId, id: string): Promise<void> {
    this.#clear();
    try {
      await installPreset(repo, id);
      this.showPresets = false;
      await this.refresh(repo);
    } catch (err) {
      this.report(err, "Could not install the preset");
    }
  }

  close(): void {
    this.lastRun = null;
    this.showPresets = false;
    this.open = false;
    this.editing = null;
    this.body = "";
  }

  private report(err: unknown, title: string): void {
    this.error =
      err instanceof CogitError ? err : new CogitError({ kind: "internal", data: String(err) });
    this.failure = title;
  }

  #clear(): void {
    this.error = null;
    this.failure = null;
  }
}

export const hooks = new HooksStore();
