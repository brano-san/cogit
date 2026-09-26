import { toggled, type FilterField } from "$lib/filter-fields";
import { forget, remember } from "$lib/filter-patterns";
import { settings } from "$stores/settings.svelte";

/** The graph filter's field, the switches under it and its remembered patterns read one
    state (F-560, F-563). */
class GraphFilterStore {
  /** What the field holds: typed, or a filter set elsewhere (File ▸ Log) written back. */
  text = $state("");

  get fields(): FilterField[] {
    return settings.current.graphFilterFields;
  }

  get patterns(): string[] {
    return settings.current.graphFilterPatterns;
  }

  /** The switches say where typed text is looked for, so they show while there is some. */
  get showsFields(): boolean {
    return this.text.trim() !== "";
  }

  toggle(field: FilterField): void {
    void settings.set("graphFilterFields", toggled(this.fields, field));
  }

  remember(text: string): void {
    void settings.set("graphFilterPatterns", remember(this.patterns, text));
  }

  forget(text: string): void {
    void settings.set("graphFilterPatterns", forget(this.patterns, text));
  }
}

export const graphFilter = new GraphFilterStore();
