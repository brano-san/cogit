import { deleteRefs, type RepoId } from "$lib/ipc";
import type { DeleteHost } from "$lib/ref-delete-run";
import { confirmation } from "$stores/confirm.svelte";
import { errors } from "$stores/errors.svelte";

/** The app's side of `runDeletion` for the repository `id`: its errors window and its
    force question. */
export function appDeleteHost(id: RepoId): DeleteHost {
  return {
    remove: (request) => deleteRefs(id, request),
    fail: (error, title) => errors.report(error, title),
    askForce: (kept) =>
      confirmation.ask({
        title: "Branches Not Fully Merged",
        message: `${kept.length} of these branches ${kept.length === 1 ? "is" : "are"} not fully merged into the current upstream/HEAD. Do you want to force delete ${kept.length === 1 ? "it" : "them"}?`,
        confirm: "Force Delete",
        items: kept,
      }),
  };
}
