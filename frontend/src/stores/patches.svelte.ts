import type { RepoId } from "$lib/ipc";
import {
  commitPatchText,
  saveCommitPatches,
  saveWorktreePatch,
  worktreePatchText,
} from "$lib/ipc/patches";
import { patchFileName } from "$lib/patches";
import { confirmation } from "./confirm.svelte";
import { errors } from "./errors.svelte";
import { notices } from "./notices.svelte";

const FILTERS = [
  { name: "Patch", extensions: ["patch", "diff"] },
  { name: "All Files", extensions: ["*"] },
];

async function copy(text: string, failure: string) {
  const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
  await writeText(text).catch((err: unknown) => errors.report(err, failure));
}

async function saveTarget(title: string, defaultPath: string): Promise<string | null> {
  const { save } = await import("@tauri-apps/plugin-dialog");
  return save({ title, defaultPath, filters: FILTERS });
}

/** Save as Patch, Copy as Patch and the Apply Patch dialog (F-948). The dialog belongs to
    the repository on screen, so leaving it closes the dialog (`lib/leaving.ts`). */
class PatchStore {
  /** Apply Patch… is open. */
  applying = $state(false);

  close(): void {
    this.applying = false;
  }

  /** One commit to one file; several to a folder, one file each, or to one file. `oids`
      in any order: git writes them oldest first. */
  async saveCommits(repo: RepoId, oids: readonly string[], subject: string | null): Promise<void> {
    const commits = [...oids];
    let target: string | null;
    let oneFile = true;
    if (commits.length === 1) {
      target = await saveTarget("Save as Patch", patchFileName(subject ?? "patch"));
    } else {
      const how = await confirmation.askWithAlternative({
        title: "Save as Patch",
        message: `Save ${commits.length} commits as patches, oldest first?`,
        confirm: "One File per Commit…",
        alternative: "One File for All…",
      });
      if (how === null) return;
      oneFile = how === "alternative";
      if (oneFile) {
        target = await saveTarget("Save as One Patch", `${commits.length}-commits.patch`);
      } else {
        const { open } = await import("@tauri-apps/plugin-dialog");
        target = await open({ title: "Save Patches to Folder", directory: true });
      }
    }
    if (!target) return;
    try {
      const written = await saveCommitPatches(repo, commits, target, oneFile);
      if (!oneFile) notices.inform("Patches saved", `${written.length} files in ${target}`);
    } catch (err) {
      errors.report(err, "Could not save the patch");
    }
  }

  async copyCommits(repo: RepoId, oids: readonly string[]): Promise<void> {
    try {
      await copy(await commitPatchText(repo, [...oids]), "Could not copy the patch");
    } catch (err) {
      errors.report(err, "Could not make the patch");
    }
  }

  /** `staged`: the Staged list, else Unstaged (untracked files as new ones). */
  async saveChanges(repo: RepoId, paths: readonly string[], staged: boolean): Promise<void> {
    const name = paths.length === 1 ? `${paths[0]?.split("/").pop() ?? "changes"}.patch` : "changes.patch";
    const target = await saveTarget(staged ? "Save Staged Changes as Patch" : "Save Changes as Patch", name);
    if (!target) return;
    await saveWorktreePatch(repo, [...paths], staged, target).catch((err) =>
      errors.report(err, "Could not save the patch"),
    );
  }

  async copyChanges(repo: RepoId, paths: readonly string[], staged: boolean): Promise<void> {
    try {
      await copy(await worktreePatchText(repo, [...paths], staged), "Could not copy the patch");
    } catch (err) {
      errors.report(err, "Could not make the patch");
    }
  }
}

export const patches = new PatchStore();
