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

/** Whether the server has commits HEAD lacks, from `ls-remote`: no fetch, no write (R-354).
    `null` — no upstream, or no such branch on the server. Never prompts. */
export async function pullProbe(root: string) {
  return unwrap(await commands.pullProbe(root));
}

