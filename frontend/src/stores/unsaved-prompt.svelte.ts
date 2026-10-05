import type { CloseChoice } from "$lib/solver-flow";

interface Pending {
  title: string;
  path: string;
  cancellable: boolean;
  settle: (choice: CloseChoice) => void;
}

/** The main window's Save / Discard / Cancel question for an editor about to lose its edits. */
class UnsavedPrompt {
  open = $state.raw<Pending | null>(null);

  ask(title: string, path: string, cancellable = true): Promise<CloseChoice> {
    this.open?.settle("cancel");
    return new Promise((resolve) => {
      this.open = {
        title,
        path,
        cancellable,
        settle: (choice) => {
          this.open = null;
          resolve(choice);
        },
      };
    });
  }

  answer(choice: CloseChoice): void {
    this.open?.settle(choice);
  }
}

export const unsavedPrompt = new UnsavedPrompt();
