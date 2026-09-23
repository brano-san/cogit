import type { RepoPhase } from "$stores/repository.svelte";

/** What a panel puts on screen. One function so that five panels cannot each decide
    differently, which is how "Graph & History (348)" came to sit over a start screen. */
export type PanelView = "start" | "opening" | "content";

export function panelView(phase: RepoPhase): PanelView {
  if (phase.kind === "closed") return "start";
  if (phase.repo !== null) return "content";
  return phase.kind === "opening" ? "opening" : "start";
}

/** What a panel body says when it has nothing of its own: the footer alone reports an open
    in progress, so a panel stays blank meanwhile rather than repeat it (#4). */
export function idleMessage(view: PanelView): string | undefined {
  return view === "start" ? "No repository open." : undefined;
}

/** The footer's repository slot: the name of what is open, or of what is being opened. */
export function footerRepository(phase: RepoPhase): string {
  if (phase.kind !== "closed" && phase.repo !== null) return phase.repo.name;
  if (phase.kind === "opening") {
    return phase.root.replace(/[/\\]+$/, "").split(/[/\\]/).pop() || phase.root;
  }
  return "No repository";
}
