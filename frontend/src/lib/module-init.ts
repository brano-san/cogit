import type { Submodule } from "$lib/ipc";

export interface ModuleScreen {
  key: string;
  /** The submodule the panels show, by key; null while they show anything else. */
  shown: string | null;
  /** The submodule an open is under way for. */
  opening: string | null;
}

/** A submodule never checked out has nothing to open; getting it changes the working
    tree, so the click is a question, never the change itself (R-149). The one on screen,
    or on its way there, is not opened again (R-520). */
export function moduleClick(state: Submodule["state"], screen?: ModuleScreen): "offer" | "open" | "stay" {
  if (state === "notInitialised") return "offer";
  if (screen && (screen.key === screen.shown || screen.key === screen.opening)) return "stay";
  return "open";
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
