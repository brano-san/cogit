import { confirmation } from "$stores/confirm.svelte";
import { hooks } from "$stores/hooks.svelte";
import { prompt } from "$stores/prompt.svelte";
import { refDialogs } from "$stores/ref-dialogs.svelte";
import { remoteOps } from "$stores/remote-ops.svelte";
import { stashDialog } from "$stores/stash-dialog.svelte";

/** What is asked or edited about the repository on screen, closed when the panels leave
    it: answered afterwards, it would act on the repository shown by then. */
export function leaveRepositoryDialogs(): void {
  confirmation.answer(false);
  prompt.cancel();
  stashDialog.cancel();
  hooks.close();
  refDialogs.close();
  remoteOps.close();
}
