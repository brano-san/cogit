import type { RepoPhase } from "$stores/repository.svelte";

/** What a panel puts on screen. One function so that five panels cannot each decide
    differently, which is how "Graph & History (348)" came to sit over a start screen. */
export type PanelView = "start" | "opening" | "content";

export function panelView(phase: RepoPhase): PanelView {
  if (phase.kind === "closed") return "start";
  if (phase.repo !== null) return "content";
  return phase.kind === "opening" ? "opening" : "start";
}
