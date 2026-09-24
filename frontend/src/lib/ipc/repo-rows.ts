import { commands } from "./bindings";
import { unwrap } from "./index";

/** Reads for list rows that are not on screen, by folder (doc/04-ipc-contract.md). */
export async function submoduleOutline(root: string, parent = "") {
  return unwrap(await commands.submoduleOutline(root, parent));
}
