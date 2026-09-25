import { tick } from "svelte";
import { layout } from "$stores/layout.svelte";

export interface CommitField {
  focus(): void;
  /** Commits if the box is ready to; `amend` ticks Amend first. */
  submit(amend: boolean): Promise<void>;
}

/** The Commit Message box as the menu, the palette and the Working Tree row reach it. It
    exists only while its panel is shown, so each of them brings the panel up first. */
class CommitBoxStore {
  #field: CommitField | null = null;

  /** Returns the detach; a box that has gone is never reached through. */
  attach(field: CommitField): () => void {
    this.#field = field;
    return () => {
      if (this.#field === field) this.#field = null;
    };
  }

  async focus(): Promise<CommitField | null> {
    if (!layout.visible("commit")) layout.togglePanel("commit");
    await tick();
    this.#field?.focus();
    return this.#field;
  }

  async commit(amend = false): Promise<void> {
    const field = await this.focus();
    await field?.submit(amend);
  }
}

export const commitBox = new CommitBoxStore();
