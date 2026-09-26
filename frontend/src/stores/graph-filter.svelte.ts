import { toggled, type FilterField } from "$lib/filter-fields";
import { settings } from "$stores/settings.svelte";

/** The graph filter's field and the switches under it read one state (F-560). */
class GraphFilterStore {
  /** What the field holds: typed, or a filter set elsewhere (File ▸ Log) written back. */
  text = $state("");

  get fields(): FilterField[] {
    return settings.current.graphFilterFields;
  }

  /** The switches say where typed text is looked for, so they show while there is some. */
  get showsFields(): boolean {
    return this.text.trim() !== "";
  }

  toggle(field: FilterField): void {
    void settings.set("graphFilterFields", toggled(this.fields, field));
  }
}

export const graphFilter = new GraphFilterStore();
