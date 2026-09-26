import {
  clampFraction,
  DEFAULT_LAYOUT,
  DEFAULT_PERSPECTIVES,
  isVisible,
  mergePerspectives,
  toggleHidden,
  type LayoutFractions,
  type PanelId,
  type Perspective,
  type PerspectiveId,
} from "$lib/perspectives";

const STORAGE_KEY = "cogit.layout.v1";

function stored(): unknown {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw === null ? null : (JSON.parse(raw) as unknown);
  } catch {
    return null;
  }
}

class LayoutStore {
  #perspectives = $state<Record<PerspectiveId, Perspective>>(mergePerspectives(stored()));
  active = $state<PerspectiveId>("main");
  maximized = $state<PanelId | null>(null);

  get fractions(): LayoutFractions {
    return this.#perspectives[this.active].fractions;
  }

  get hidden(): readonly PanelId[] {
    return this.#perspectives[this.active].hidden;
  }

  visible(panel: PanelId): boolean {
    return isVisible(this.#perspectives[this.active], this.maximized, panel);
  }

  nudge(key: keyof LayoutFractions, delta: number): void {
    this.set(key, this.fractions[key] + delta);
  }

  set(key: keyof LayoutFractions, value: number): void {
    this.#perspectives[this.active].fractions[key] = clampFraction(value);
    this.persist();
  }

  /** Switching keeps each perspective's own sizes; that is the point of having them. */
  switch(id: PerspectiveId): void {
    this.maximized = null;
    this.active = id;
  }

  toggleMaximized(panel: PanelId): void {
    this.maximized = this.maximized === panel ? null : panel;
  }

  /** Does what the tick in View promised: with a panel maximised the ticks read what is
      on screen, so an unticked panel is shown, not hidden unseen. */
  togglePanel(panel: PanelId): void {
    const show = !this.visible(panel);
    this.maximized = null;
    if (show === this.hidden.includes(panel)) {
      this.#perspectives[this.active].hidden = toggleHidden(this.hidden, panel);
    }
    this.persist();
  }

  reset(): void {
    this.#perspectives[this.active] = structuredClone(DEFAULT_PERSPECTIVES[this.active]);
    this.maximized = null;
    this.persist();
  }

  resetOne(key: keyof LayoutFractions): void {
    this.set(key, DEFAULT_LAYOUT[key]);
  }

  private persist(): void {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(this.#perspectives));
    } catch {
      // A blocked localStorage costs the layout on restart, nothing more.
    }
  }
}

export const layout = new LayoutStore();
