/** What the graph has selected: a commit id, or `null` for the Working Tree row. */
export type Selected = string | null;

/** Home (F-562): from the Working Tree to HEAD's commit, from anywhere else to the Working
    Tree. Without a HEAD commit (an empty repository) Home stays on the Working Tree; with
    no Working Tree row (a clean tree, F-561) it always goes to HEAD. */
export function homeTarget(selected: Selected, head: string | null, workingTreeRow = true): Selected {
  if (!workingTreeRow && head !== null) return head;
  return selected === null ? head : null;
}

/** The selections before the current one, for Back (F-562). Only moving on records one;
    going back does not, so Back walks further back each time, like a browser without
    Forward. */
export class SelectionHistory {
  static readonly LIMIT = 100;
  #before: Selected[] = [];
  #current: Selected | undefined = undefined;

  get canGoBack(): boolean {
    return this.#before.length > 0;
  }

  /** The graph now shows `selected`; the first call only says where it starts. */
  visit(selected: Selected): void {
    if (this.#current === undefined) {
      this.#current = selected;
      return;
    }
    if (selected === this.#current) return;
    this.#before.push(this.#current);
    if (this.#before.length > SelectionHistory.LIMIT) this.#before.shift();
    this.#current = selected;
  }

  /** The selection to go back to, now the current one; `undefined` when there is none. */
  back(): Selected | undefined {
    if (this.#before.length === 0) return undefined;
    const target = this.#before.pop() as Selected;
    this.#current = target;
    return target;
  }
}
