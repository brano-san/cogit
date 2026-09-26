import {
  branchOptions,
  cloneRequest,
  destinationToAsk,
  directoryProblem,
  initialBranch,
  limitProblem,
  sourceProblem,
  type ClonePage,
  type Destination,
} from "$lib/clone";
import { toCogitError, type CogitError } from "$lib/ipc";
import {
  clipboardRepositoryUrl,
  cloneDestination,
  remoteBranches,
  type CloneRequest,
  type RemoteBranches,
} from "$lib/ipc/clone";
import { pathFromUrl } from "$lib/remote-dialogs";

export type CloneCheck =
  | { state: "idle" }
  | { state: "checking"; source: string }
  | { state: "passed"; source: string; listing: RemoteBranches }
  | { state: "failed"; source: string; error: CogitError }
  /** Continue Without Check: git asks for credentials during the clone itself. */
  | { state: "skipped"; source: string };

/** Repository ▸ Clone… (F-575): the three pages and what each of them holds. */
export class CloneWizard {
  open = $state(false);
  page = $state<ClonePage>("repository");
  source = $state("");
  check = $state.raw<CloneCheck>({ state: "idle" });
  submodules = $state(true);
  allBranches = $state(true);
  skipLarge = $state(false);
  limitMb = $state("1");
  branch = $state<string | null>(null);
  parent = $state("");
  name = $state("");
  seen = $state.raw<Destination | null>(null);

  /** What the clipboard put in: not typed, so closing does not ask about it. */
  #prefilled = "";
  #nameTyped = false;
  /** A wizard closed or opened again drops the answers still on their way. */
  #session = 0;

  async start(parent: string): Promise<void> {
    const session = ++this.#session;
    this.page = "repository";
    this.source = "";
    this.check = { state: "idle" };
    this.submodules = true;
    this.allBranches = true;
    this.skipLarge = false;
    this.limitMb = "1";
    this.branch = null;
    this.parent = parent;
    this.name = "";
    this.seen = null;
    this.#prefilled = "";
    this.#nameTyped = false;
    this.open = true;
    const url = await clipboardRepositoryUrl().catch(() => null);
    if (session !== this.#session || this.source !== "" || url === null) return;
    this.#prefilled = url;
    this.setSource(url);
  }

  close(): void {
    this.#session += 1;
    this.open = false;
  }

  setSource(text: string): void {
    this.source = text;
    if (this.check.state !== "idle") this.check = { state: "idle" };
    if (!this.#nameTyped) this.name = pathFromUrl(text);
  }

  setParent(path: string): void {
    this.parent = path;
    void this.#ask();
  }

  setName(name: string): void {
    this.name = name;
    this.#nameTyped = true;
    void this.#ask();
  }

  get listing(): RemoteBranches | null {
    return this.check.state === "passed" ? this.check.listing : null;
  }

  get branches(): [string, string][] {
    return this.listing ? branchOptions(this.listing) : [];
  }

  /** Why the branch cannot be chosen, or `null` when it can. */
  get branchReason(): string | null {
    if (this.listing === null) return "The branches are listed once the check succeeds";
    if (this.listing.branches.length === 0) return "The repository has no branches yet";
    return null;
  }

  /** What keeps Next or Finish inactive on this page. */
  get problem(): string | null {
    switch (this.page) {
      case "repository":
        return (
          sourceProblem(this.source) ??
          (this.check.state === "checking" ? "Checking access to the repository…" : null)
        );
      case "selection":
        return limitProblem(this.skipLarge, this.limitMb);
      case "directory":
        return directoryProblem({ parent: this.parent, name: this.name, seen: this.seen });
    }
  }

  /** The check failed for what is in the field now. */
  get failed(): CogitError | null {
    return this.check.state === "failed" && this.check.source === this.source.trim() ? this.check.error : null;
  }

  get dirty(): boolean {
    return this.page !== "repository" || this.source.trim() !== this.#prefilled.trim();
  }

  async next(): Promise<void> {
    if (this.problem !== null) return;
    if (this.page === "repository") {
      const passed = this.check.state === "passed" && this.check.source === this.source.trim();
      if (passed) this.page = "selection";
      else await this.#check();
    } else if (this.page === "selection") {
      this.page = "directory";
      void this.#ask();
    }
  }

  continueUnchecked(): void {
    if (this.failed === null) return;
    this.check = { state: "skipped", source: this.source.trim() };
    this.branch = null;
    this.page = "selection";
  }

  back(): void {
    if (this.page === "directory") this.page = "selection";
    else if (this.page === "selection") this.page = "repository";
  }

  /** The request Finish sends, or `null` while something keeps it inactive. */
  finish(): CloneRequest | null {
    if (this.page !== "directory" || this.problem !== null) return null;
    return cloneRequest({
      source: this.source,
      submodules: this.submodules,
      allBranches: this.allBranches,
      branch: this.branch,
      listing: this.listing,
      skipLarge: this.skipLarge,
      limitMb: this.limitMb,
      parent: this.parent,
      name: this.name,
    });
  }

  async #check(): Promise<void> {
    const source = this.source.trim();
    const session = this.#session;
    this.check = { state: "checking", source };
    const stale = () => session !== this.#session || this.source.trim() !== source;
    try {
      const listing = await remoteBranches(source);
      if (stale()) return;
      this.check = { state: "passed", source, listing };
      this.branch = initialBranch(listing);
      this.page = "selection";
    } catch (err) {
      if (!stale()) this.check = { state: "failed", source, error: toCogitError(err) };
    }
  }

  async #ask(): Promise<void> {
    const path = destinationToAsk(this.parent, this.name);
    if (path === null) return;
    const session = this.#session;
    const kind = await cloneDestination(path).catch(() => "unreadable" as const);
    if (session === this.#session && destinationToAsk(this.parent, this.name) === path) {
      this.seen = { path, kind };
    }
  }
}

export const cloneWizard = new CloneWizard();
