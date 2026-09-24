export interface PromptRequest {
  title: string;
  label: string;
  value?: string;
  /** A list to pick from instead of a free text field. */
  choices?: string[];
  confirm: string;
  /** Why the value will not do, or null. Every caller names its rule (`$lib/names`): a
      default one checked group names, preset names and an optional tag as branch names. */
  validate: (value: string) => string | null;
}

interface Pending extends PromptRequest {
  settle: (value: string | null) => void;
}

/** One text prompt at a time, asked for and awaited rather than wired through a callback:
    the caller reads like the sentence it is, and nobody has to remember to close it. */
class PromptStore {
  open = $state.raw<Pending | null>(null);

  /** Resolves with what was typed, or null when the user backed out. */
  ask(request: PromptRequest): Promise<string | null> {
    // A second ask while one is up would strand the first caller's promise.
    this.open?.settle(null);
    return new Promise((resolve) => {
      this.open = {
        ...request,
        settle: (value) => {
          this.open = null;
          resolve(value);
        },
      };
    });
  }

  accept(value: string): void {
    this.open?.settle(value);
  }

  cancel(): void {
    this.open?.settle(null);
  }
}

export const prompt = new PromptStore();
