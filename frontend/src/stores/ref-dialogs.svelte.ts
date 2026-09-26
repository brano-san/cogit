import type { PushSource } from "$lib/push-to";
import type { CheckoutOffer } from "$lib/ref-checkout";

/** The dialogs of the graph and Branches menus (`RefActions`). Each names a commit or a
    branch of the repository on screen, so leaving it closes them all (`lib/leaving.ts`). */
class RefDialogs {
  tag = $state.raw<{ oid: string; subject: string } | null>(null);
  push = $state.raw<PushSource | null>(null);
  reset = $state.raw<{ oid: string; subject: string; moving: string } | null>(null);
  message = $state.raw<{ oid: string; message: string; parents: string[] } | null>(null);
  author = $state.raw<{ oid: string; name: string; email: string } | null>(null);
  upstream = $state.raw<{ branch: string; current: string | null } | null>(null);
  checkout = $state.raw<CheckoutOffer | null>(null);
  applyStash = $state.raw<{ index: number; message: string } | null>(null);

  close(): void {
    this.tag = null;
    this.push = null;
    this.reset = null;
    this.message = null;
    this.author = null;
    this.upstream = null;
    this.checkout = null;
    this.applyStash = null;
  }
}

export const refDialogs = new RefDialogs();
