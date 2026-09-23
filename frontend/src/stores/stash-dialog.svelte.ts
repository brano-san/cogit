import type { StashChoice } from "$lib/stash-modes";

type Pending =
  | { kind: "create"; settle: (choice: StashChoice | null) => void }
  | { kind: "selection"; paths: readonly string[]; settle: (message: string | null) => void };

/** The two stash dialogs, asked for and awaited like `prompt`: one open at a time. */
class StashDialogStore {
  open = $state.raw<Pending | null>(null);

  /** Stash ▸ the main button: a name and one of the three modes, or null. */
  create(): Promise<StashChoice | null> {
    this.#dismiss();
    return new Promise((resolve) => {
      this.open = {
        kind: "create",
        settle: (choice) => {
          this.open = null;
          resolve(choice);
        },
      };
    });
  }

  /** Stash Selection: the files listed for a yes; the message, possibly empty, or null. */
  selection(paths: readonly string[]): Promise<string | null> {
    this.#dismiss();
    return new Promise((resolve) => {
      this.open = {
        kind: "selection",
        paths,
        settle: (message) => {
          this.open = null;
          resolve(message);
        },
      };
    });
  }

  cancel(): void {
    this.#dismiss();
  }

  /** A second ask while one is up would strand the first caller's promise. */
  #dismiss(): void {
    this.open?.settle(null);
  }
}

export const stashDialog = new StashDialogStore();
