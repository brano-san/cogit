import {
  asCogitError,
  commandNotice,
  errorNotice,
  pushError,
  queueOf,
  warningNotice,
  type Notice,
} from "$lib/notices";
import { commandOutcome, type CommandNotice } from "$lib/ipc";
import { health } from "$stores/health.svelte";
import { untrack } from "svelte";

/** The one notification window: failures and repository warnings in one queue, errors
    first (doc/12-risks.md, R-178). */
class NoticeStore {
  #errors = $state.raw<Notice[]>([]);
  #results = $state.raw<Notice[]>([]);
  #key = $state<string | null>(null);
  #index = $state(0);
  #seq = 0;
  #loading = new Set<number>();

  get all(): Notice[] {
    return queueOf(this.#errors, health.warnings.map(warningNotice), this.#results);
  }

  get at(): number {
    const all = this.all;
    const found = all.findIndex((notice) => notice.key === this.#key);
    return found >= 0 ? found : Math.max(0, Math.min(this.#index, all.length - 1));
  }

  get current(): Notice | undefined {
    return this.all[this.at];
  }

  get errorCount(): number {
    return this.#errors.length;
  }

  /** Called from effects (App reports each store's error that way). Untracked, or the
      effect would depend on the queue it writes: every report ran it again as a new
      notice, and a dismissed one came straight back. */
  report(error: unknown, title: string): void {
    untrack(() => {
      const cogit = asCogitError(error);
      // The user stopped it; the journal's warning already says so.
      if (!cogit || cogit.detail.kind === "cancelled") return;
      this.#seq += 1;
      this.#queueError(errorNotice(cogit, title, this.#seq));
    });
  }

  /** How an operation ended, behind every error and warning; shown when nothing else is. */
  inform(title: string, body: string): void {
    untrack(() => {
      this.#seq += 1;
      const notice: Notice = {
        key: `info:${this.#seq}`,
        severity: "info",
        title,
        body,
        report: `${title}\n${body}`,
        repeats: 1,
      };
      const shown = this.current;
      this.#results = [...this.#results, notice];
      if (!shown) this.#key = notice.key;
    });
  }

  message(text: string, title: string): void {
    this.report(new Error(text), title);
  }

  /** A failed git command. It arrives twice — as the event and as the rejected call —
      and is queued once. */
  async command(event: CommandNotice): Promise<void> {
    const key = `command:${event.id}`;
    const known = untrack(() => this.#errors.some((held) => held.key === key));
    if (known || this.#loading.has(event.id)) return;

    this.#loading.add(event.id);
    const run = await commandOutcome(event.id).catch(() => null);
    this.#loading.delete(event.id);
    this.#queueError(
      commandNotice(run ?? { ...event, command: "", exitCode: null, stdout: "", stderr: "" }),
    );
  }

  step(delta: -1 | 1): void {
    const wanted = this.at + delta;
    const next = this.all[wanted];
    if (!next) return;
    this.#key = next.key;
    this.#index = wanted;
  }

  /** Closes the entry on screen and shows the one after it; the last one closes the window.
      A repository warning closed this way comes back on the next open (Remind me later). */
  dismiss(): void {
    const current = this.current;
    if (!current) return;
    const at = this.at;
    if (current.severity === "error") {
      this.#errors = this.#errors.filter((held) => held.key !== current.key);
    } else if (current.severity === "info") {
      this.#results = this.#results.filter((held) => held.key !== current.key);
    } else if (current.warning) {
      health.remindLater(current.warning);
    }
    this.#follow(at);
  }

  async ignore(): Promise<void> {
    const current = this.current;
    if (!current?.warning) return;
    const at = this.at;
    await health.ignore(current.warning);
    this.#follow(at);
  }

  dismissAll(): void {
    this.#errors = [];
    this.#results = [];
    this.#key = null;
    this.#index = 0;
  }

  /** Ahead of everything, whatever was on screen: an error is never behind a warning. */
  #queueError(notice: Notice): void {
    if (this.#errors.some((held) => held.key === notice.key)) return;
    this.#errors = pushError(this.#errors, notice);
    this.#key = this.#errors[0]?.key ?? null;
    this.#index = 0;
  }

  #follow(at: number): void {
    const all = this.all;
    const index = Math.max(0, Math.min(at, all.length - 1));
    this.#key = all[index]?.key ?? null;
    this.#index = index;
  }
}

export const notices = new NoticeStore();
