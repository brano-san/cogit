import { folderKind, type FolderKind } from "$lib/ipc/clone";
import {
  checkAvailability,
  keepSelection,
  moveSelection,
  selectionAfterRemoval,
  type Availability,
  type MruRow,
  type Step,
} from "$lib/welcome";

/** A dead share must not leave a row "checking" for ever, nor hold the dialog. */
const CHECK_TIMEOUT_MS = 4000;

type Probe = (path: string) => Promise<FolderKind>;

/** The Welcome dialog (F-586): open or not, the filter, the selected recent repository (by
    path, so a filter or a removal cannot shift it onto another row), and which turned out gone. */
export class WelcomeDialog {
  open = $state(false);
  selected = $state<string | null>(null);
  query = $state("");
  availability = $state.raw<ReadonlyMap<string, Availability>>(new Map());

  #probe: Probe;
  #timeoutMs: number;
  #checking: AbortController | null = null;

  constructor(probe: Probe = folderKind, timeoutMs = CHECK_TIMEOUT_MS) {
    this.#probe = probe;
    this.#timeoutMs = timeoutMs;
  }

  /** Rows are on screen at once; each is marked as its check ends. */
  show(paths: readonly string[]): void {
    this.#checking?.abort();
    this.open = true;
    this.query = "";
    this.selected = paths[0] ?? null;
    this.availability = new Map(paths.map((path) => [path, "checking"]));
    const controller = new AbortController();
    this.#checking = controller;
    void checkAvailability(paths, this.#probe, {
      timeoutMs: this.#timeoutMs,
      signal: controller.signal,
      onResult: (path, value) => {
        if (!this.availability.has(path)) return;
        this.availability = new Map(this.availability).set(path, value);
      },
    });
  }

  close(): void {
    this.#checking?.abort();
    this.#checking = null;
    this.open = false;
  }

  select(path: string): void {
    this.selected = path;
  }

  move(step: Step, shown: readonly MruRow[]): void {
    this.selected = moveSelection(shown, this.selected, step);
  }

  /** `shown`: the rows the new filter leaves. */
  filter(query: string, shown: readonly MruRow[]): void {
    this.query = query;
    this.selected = keepSelection(shown, this.selected);
  }

  /** Called before `paths` leave the list, with the rows shown now. */
  removing(paths: readonly string[], shown: readonly MruRow[]): void {
    this.selected = selectionAfterRemoval(shown, this.selected, new Set(paths));
  }
}

export const welcome = new WelcomeDialog();
