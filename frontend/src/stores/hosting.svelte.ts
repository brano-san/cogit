import {
  addHostingAccount,
  checkHostingAccount,
  hostingAccountFor,
  hostingAccounts,
  removeHostingAccount,
  type HostingAccount,
  type HostingMatch,
  type NewHostingAccount,
  type StoreState,
} from "$lib/ipc";
import { providerLabel, serverOf } from "$lib/hosting";
import { confirmation } from "$stores/confirm.svelte";
import { masterPassword } from "$stores/master-password.svelte";
import { notices } from "$stores/notices.svelte";

export type Checked = { state: "checking" } | { state: "ok"; login: string } | { state: "failed"; error: string };

function message(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  return JSON.stringify(err);
}

class HostingStore {
  accounts = $state.raw<HostingAccount[]>([]);
  state = $state<StoreState>("missing");
  checked = $state.raw<Record<string, Checked>>({});
  adding = $state(false);
  addError = $state<string | null>(null);
  busy = $state(false);
  lookup = $state.raw<{ url: string; found: HostingMatch | null; error: string | null } | null>(null);

  async refresh(): Promise<void> {
    try {
      const listed = await hostingAccounts();
      this.accounts = listed.accounts;
      this.state = listed.state;
    } catch (err) {
      notices.report(err, "Could not read the hosting accounts");
    }
  }

  async unlock(): Promise<void> {
    try {
      await masterPassword.unlock();
    } catch (err) {
      notices.report(err, "Could not unlock the password store");
    }
    await this.refresh();
  }

  async openAdd(): Promise<void> {
    try {
      if (!(await masterPassword.ready())) return;
    } catch (err) {
      notices.report(err, "Could not open the password store");
      return;
    }
    this.addError = null;
    this.adding = true;
  }

  closeAdd(): void {
    if (this.busy) return;
    this.adding = false;
    this.addError = null;
  }

  async add(account: NewHostingAccount): Promise<void> {
    this.busy = true;
    this.addError = null;
    try {
      const added = await addHostingAccount(account);
      this.checked = { ...this.checked, [added.id]: { state: "ok", login: added.login } };
      this.adding = false;
    } catch (err) {
      this.addError = message(err);
    } finally {
      this.busy = false;
    }
    await this.refresh();
  }

  async check(account: HostingAccount): Promise<void> {
    this.checked = { ...this.checked, [account.id]: { state: "checking" } };
    let result: Checked;
    try {
      result = { state: "ok", login: await checkHostingAccount(account.id) };
    } catch (err) {
      result = { state: "failed", error: message(err) };
    }
    this.checked = { ...this.checked, [account.id]: result };
    await this.refresh();
  }

  async remove(account: HostingAccount): Promise<void> {
    const go = await confirmation.ask({
      title: "Remove Hosting Account",
      message: `Remove the ${providerLabel(account.provider)} account ${account.login} on ${serverOf(account)}? Its token is deleted from Cogit's password store; the token itself stays valid until you revoke it on the server.`,
      confirm: "Remove",
      warning: true,
    });
    if (!go) return;
    try {
      await removeHostingAccount(account.id);
    } catch (err) {
      notices.report(err, "Could not remove the hosting account");
    }
    const { [account.id]: _gone, ...rest } = this.checked;
    this.checked = rest;
    await this.refresh();
  }

  async find(url: string): Promise<void> {
    const text = url.trim();
    if (text === "") {
      this.lookup = null;
      return;
    }
    try {
      this.lookup = { url: text, found: await hostingAccountFor(text), error: null };
    } catch (err) {
      this.lookup = { url: text, found: null, error: message(err) };
    }
  }
}

export const hosting = new HostingStore();
