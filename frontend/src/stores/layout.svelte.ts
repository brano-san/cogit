import {
  clampFraction,
  DEFAULT_PERSPECTIVES,
  isVisible,
  toggleHidden,
  type LayoutFractions,
  type PanelId,
  type PerspectiveId,
} from "$lib/perspectives";

import { WINDOW_LAYOUTS, windowProfiles, type LayoutProfile, type WindowLayout } from "$lib/window-layout";
import { readKey, writeKey } from "$lib/settings-file";
import { settings } from "./settings.svelte";
import { errors } from "./errors.svelte";

const PROFILE_KEY = "windowLayouts";
const STORAGE_KEY = "cogit.layout.v1";

function stored(): unknown {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw === null ? null : (JSON.parse(raw) as unknown);
  } catch {
    return null;
  }
}

export class LayoutStore {
  #profiles = $state(windowProfiles(null, null));
  #maximized = $state<Partial<Record<LayoutProfile, PanelId | null>>>({});
  #writes: Promise<void> = Promise.resolve();
  #revision = 0;
  #available = false;
  active = $state<PerspectiveId>("main");

  constructor(private choice: () => WindowLayout, private legacy = stored()) {
    this.#profiles = windowProfiles(null, legacy);
  }

  async load(): Promise<void> {
    const saved = await readKey<unknown>(PROFILE_KEY);
    this.#profiles = windowProfiles(saved, this.legacy);
    this.#available = true;
  }

  get activeLayout(): WindowLayout {
    return this.choice();
  }

  get arrangement() {
    return WINDOW_LAYOUTS[this.activeLayout];
  }

  private get profile(): LayoutProfile {
    const layout = this.activeLayout;
    return layout === "standard" ? this.active : layout;
  }

  private get defaults() {
    const profile = this.profile;
    return profile === "main" || profile === "review" ? DEFAULT_PERSPECTIVES[profile] : WINDOW_LAYOUTS[profile].defaults;
  }

  get maximized(): PanelId | null {
    return this.#maximized[this.profile] ?? null;
  }

  private set maximized(value: PanelId | null) {
    this.#maximized[this.profile] = value;
  }

  get fractions(): LayoutFractions {
    return this.#profiles[this.profile].fractions;
  }

  get hidden(): readonly PanelId[] {
    return this.#profiles[this.profile].hidden;
  }

  visible(panel: PanelId): boolean {
    return isVisible(this.#profiles[this.profile], this.maximized, panel);
  }

  nudge(key: keyof LayoutFractions, delta: number): void {
    this.set(key, this.fractions[key] + delta);
  }

  set(key: keyof LayoutFractions, value: number): void {
    this.#profiles[this.profile].fractions[key] = clampFraction(value);
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
      this.#profiles[this.profile].hidden = toggleHidden(this.hidden, panel);
    }
    this.persist();
  }

  reset(): void {
    this.#profiles[this.profile] = structuredClone(this.defaults);
    this.maximized = null;
    this.persist();
  }

  resetOne(key: keyof LayoutFractions): void {
    this.set(key, this.defaults.fractions[key]);
  }

  resetSizes(): void {
    this.#profiles[this.profile].fractions = { ...this.defaults.fractions };
    this.persist();
  }

  saved(): Promise<void> {
    return this.#writes;
  }

  private persist(): void {
    if (!this.#available) return;
    const revision = ++this.#revision;
    this.#writes = this.#writes.then(async () => {
      if (revision === this.#revision) await writeKey(PROFILE_KEY, $state.snapshot(this.#profiles));
    }).catch((err) => errors.report(err, "Could not save the window layout"));
  }
}

export const layout = new LayoutStore(() => settings.current.activeLayout);

export function setWindowLayout(id: WindowLayout): void {
  if (id !== layout.activeLayout) void settings.setActiveLayout(id);
}

export function resetWindowLayout(): void {
  layout.resetSizes();
}
