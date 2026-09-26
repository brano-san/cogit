/** The row a context menu is open on, marked while the menu is up; the selection stays
    where it was (R-545). The native menu answers once it closes. */
export class MenuRow {
  key = $state<string | null>(null);
  #ticket = 0;

  async hold<T>(key: string, menu: () => Promise<T>): Promise<T> {
    const ticket = ++this.#ticket;
    this.key = key;
    try {
      return await menu();
    } finally {
      if (ticket === this.#ticket) this.key = null;
    }
  }
}

/** Rows of Repositories by root, a submodule node by `<root of its top>/<key>`. */
export const repoMenuRow = new MenuRow();
