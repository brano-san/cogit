import type { RepoPhase } from "$stores/repository.svelte";

/** What a panel puts on screen. One function so that five panels cannot each decide
    differently, which is how "Graph & History (348)" came to sit over a start screen. */
export type PanelView = "start" | "opening" | "content";

export function panelView(phase: RepoPhase): PanelView {
  if (phase.kind === "closed") return "start";
  if (phase.repo !== null) return "content";
  return phase.kind === "opening" ? "opening" : "start";
}

/** The window's one empty state (StartScreen): up while nothing is open or opening, and not
    before the session has been restored, when "nothing open" is only the first frame.
    The panels say nothing of their own; the footer alone reports an open in progress (#4). */
export function emptyStateVisible(phase: RepoPhase, ready: boolean): boolean {
  return ready && panelView(phase) === "start";
}

/** The footer's repository slot: the name of what is open, or of what is being opened. */
export function footerRepository(phase: RepoPhase): string {
  if (phase.kind !== "closed" && phase.repo !== null) return phase.repo.name;
  if (phase.kind === "opening") {
    return phase.root.replace(/[/\\]+$/, "").split(/[/\\]/).pop() || phase.root;
  }
  return "No repository";
}
