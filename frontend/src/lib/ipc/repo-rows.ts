import { commands } from "./bindings";
import { unwrap } from "./index";

/** Reads for list rows that are not on screen, by folder (doc/04-ipc-contract.md). */
export async function submoduleOutline(root: string, parent = "") {
  return unwrap(await commands.submoduleOutline(root, parent));
}

export type { RepoPulse } from "./bindings";

export async function repoPulse(root: string) {
  return unwrap(await commands.repoPulse(root));
}

/** Never prompts; a failure is logged by the backend and returned here only as a refusal. */
export async function backgroundFetch(root: string) {
  unwrap(await commands.backgroundFetch(root));
}
