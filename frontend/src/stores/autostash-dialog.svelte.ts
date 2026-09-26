export interface AutostashAnswer {
  /** Drop the stash once it applied cleanly; off, it stays in the list either way. */
  drop: boolean;
}

interface Pending {
  question: string;
  settle: (answer: AutostashAnswer | null) => void;
}

/** The offer to carry local changes over a checkout (item 46), awaited like `confirmation`. */
class AutostashDialogStore {
  open = $state.raw<Pending | null>(null);

  ask(question: string): Promise<AutostashAnswer | null> {
    this.open?.settle(null);
    return new Promise((resolve) => {
      this.open = {
        question,
        settle: (answer) => {
          this.open = null;
          resolve(answer);
        },
      };
    });
  }

  answer(answer: AutostashAnswer | null): void {
    this.open?.settle(answer);
  }
}

export const autostashDialog = new AutostashDialogStore();
