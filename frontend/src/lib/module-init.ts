import type { Submodule } from "$lib/ipc";

/** A submodule never checked out has nothing to open; getting it changes the working
    tree, so the click is a question, never the change itself (R-149). */
export function moduleClick(state: Submodule["state"]): "offer" | "open" {
  return state === "notInitialised" ? "offer" : "open";
}

export interface InitialiseDeps {
  ask: (key: string) => Promise<boolean>;
  update: (key: string) => Promise<void>;
}

/** Offers `git submodule update --init` for one node at a time: the clicks of a
    double-click arrive while the first question is still open. */
export class ModuleInitialiser {
  readonly #deps: InitialiseDeps;
  readonly #asking = new Set<string>();

  constructor(deps: InitialiseDeps) {
    this.#deps = deps;
  }

  async offer(key: string): Promise<void> {
    if (this.#asking.has(key)) return;
    this.#asking.add(key);
    try {
      if (await this.#deps.ask(key)) await this.#deps.update(key);
    } finally {
      this.#asking.delete(key);
    }
  }
}
