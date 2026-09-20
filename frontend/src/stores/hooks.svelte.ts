import {
  bypassLog,
  installPreset,
  listPresets,
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
  error = $state<CogitError | null>(null);
  lastRun = $state.raw<HookRun | null>(null);
  running = $state(false);
  bypasses = $state.raw<Bypass[]>([]);
  presets = $state.raw<PresetStatus[]>([]);
  showPresets = $state(false);

  async refresh(repo: RepoId): Promise<void> {
    try {
      this.overview = await listHooks(repo);
      this.bypasses = await bypassLog(repo);
      this.presets = await listPresets();
    } catch (err) {
      this.report(err);
    }
  }

  async show(repo: RepoId): Promise<void> {
    this.open = true;
    await this.refresh(repo);
  }

  async edit(repo: RepoId, name: string): Promise<void> {
    this.editing = name;
    const present = this.overview?.hooks.find((hook) => hook.name === name);
    this.body =
      present && present.state !== "missing"
        ? await readHook(repo, name).catch(() => "")
        : "#!/bin/sh\nset -e\n\n";
  }

  async save(repo: RepoId): Promise<void> {
    if (this.editing === null) return;
    try {
      await writeHook(repo, this.editing, this.body);
      this.editing = null;
      await this.refresh(repo);
    } catch (err) {
      this.report(err);
    }
  }

  async toggle(repo: RepoId, name: string, enabled: boolean): Promise<void> {
    try {
      await setHookEnabled(repo, name, enabled);
      await this.refresh(repo);
    } catch (err) {
      this.report(err);
    }
  }

  async adopt(repo: RepoId, path: string): Promise<void> {
    try {
      await useHooksPath(repo, path);
      await this.refresh(repo);
    } catch (err) {
      this.report(err);
    }
  }

  /** Runs the hook alone, with no commit behind it. */
  async dryRun(repo: RepoId, name: string): Promise<void> {
    this.running = true;
    this.lastRun = null;
    try {
      this.lastRun = await runHook(repo, name);
    } catch (err) {
      this.report(err);
    } finally {
      this.running = false;
    }
  }

  async install(repo: RepoId, id: string): Promise<void> {
    try {
      await installPreset(repo, id);
      this.showPresets = false;
      await this.refresh(repo);
    } catch (err) {
      this.report(err);
    }
  }

  close(): void {
    this.lastRun = null;
    this.showPresets = false;
    this.open = false;
    this.editing = null;
    this.body = "";
  }

  private report(err: unknown): void {
    this.error =
      err instanceof CogitError ? err : new CogitError({ kind: "internal", data: String(err) });
  }
}

export const hooks = new HooksStore();
