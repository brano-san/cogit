import { commands, type DiffSpec, type RepoId, type Shape } from "./bindings";
import { unwrap } from "./index";

export type { EditableFile, SaveOutcome, Shape } from "./bindings";

/** The working file and the diff's left side for the editor, or why it cannot be edited. */
export async function readEditable(repo: RepoId, spec: DiffSpec, path: string) {
  return unwrap(await commands.readEditable(repo, spec, path));
}

/** Writes `text` in the file's own encoding and endings; `changedOnDisk` unless `force`. */
export async function saveEditable(
  repo: RepoId,
  path: string,
  text: string,
  shape: Shape,
  stamp: string,
  force: boolean,
) {
  return unwrap(await commands.saveEditable(repo, path, text, shape, stamp, force));
}
