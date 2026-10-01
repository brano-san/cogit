import { folderKind, type FolderKind } from "$lib/ipc/clone";
import {
  checkAvailability,
  clampSelection,
  defaultSelection,
  moveSelection,
  selectOption,
  type Availability,
  type WelcomeOption,
  type WelcomeSelection,
} from "$lib/welcome";

/** A dead share must not leave a row "checking" for ever, nor hold the dialog. */
const CHECK_TIMEOUT_MS = 4000;

type Probe = (path: string) => Promise<FolderKind>;

/** The Welcome dialog (F-586): open or not, what is selected, and which recent
    repositories turned out to be gone. */
export class WelcomeDialog {
  open = $state(false);
  selection = $state.raw<WelcomeSelection>({ option: 1, row: null });
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
    this.selection = defaultSelection(paths.length);
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

  choose(option: WelcomeOption, rowCount: number): void {
    this.selection = selectOption(this.selection, option, rowCount);
  }

  chooseRow(row: number): void {
    this.selection = { option: 3, row };
  }

  move(dir: "up" | "down", rowCount: number): void {
    this.selection = moveSelection(this.selection, dir, rowCount);
  }

  /** The list lost a row: the selection stays on the one that took its place. */
  listChanged(rowCount: number): void {
    this.selection = clampSelection(this.selection, rowCount);
  }
}

export const welcome = new WelcomeDialog();
