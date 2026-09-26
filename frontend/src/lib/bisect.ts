import { SEPARATOR, offer, submenu } from "./context-menu";
import { shortOid } from "./format";
import type { ContextItem, RepoState } from "./ipc";
import type { BisectMark, BisectState } from "./ipc/bisect";
import type { PaletteCommand } from "./palette";
import type { Banner, BannerAction } from "./repo-state";

/** Bisect after SmartGit (https://docs.syntevo.com/SmartGit/Latest/Manual/GUI/Bisect):
    a bad and a good commit, then git checks out the one between them to test. */
export function bisectOf(state: RepoState | null | undefined): BisectState | null {
  return state?.kind === "bisecting" ? state.bisect : null;
}

/** Where the search stands. `stuck`: only skipped commits are left to test. */
export type BisectPhase = "waiting" | "needGood" | "needBad" | "testing" | "found" | "stuck";

export function bisectPhase(bisect: BisectState): BisectPhase {
  if (bisect.firstBad) return "found";
  if (bisect.candidates.length > 0) return "stuck";
  const bad = bisect.bad !== null;
  const good = bisect.good.length > 0;
  if (bad && good) return "testing";
  if (bad) return "needGood";
  return good ? "needBad" : "waiting";
}

const ONGOING: ReadonlySet<RepoState["kind"]> = new Set([
  "merging",
  "rebasing",
  "cherryPicking",
  "reverting",
  "applyingPatches",
]);

/** Why no bisect can start, or null. Lower case: it goes in a menu row's parentheses. */
export function startBlocked(state: RepoState | null | undefined): string | null {
  if (!state) return "no repository";
  if (state.kind === "bisecting") return "a bisect is in progress";
  if (state.kind === "bare") return "a bare repository";
  if (state.kind === "empty") return "no commits yet";
  return ONGOING.has(state.kind) ? "another operation is in progress" : null;
}

/** What a mark of `oid` would repeat, or null. */
function markBlocked(bisect: BisectState | null, mark: BisectMark, oid: string | null): string | null {
  if (!bisect) return "no bisect in progress";
  if (oid === null) return null;
  if (mark === "good") return bisect.good.includes(oid) ? "already good" : null;
  if (mark === "bad") return bisect.bad === oid ? "already bad" : null;
  return bisect.skipped.includes(oid) ? "already skipped" : null;
}

export const BISECT_MENU_PREFIX = "bisect:";

/** The chosen row, from a `bisect:<action>:<oid>` id; null for any other id. */
export function bisectMenuChoice(id: string): { action: BisectAction; oid: string } | null {
  if (!id.startsWith(BISECT_MENU_PREFIX)) return null;
  const [action, oid] = id.slice(BISECT_MENU_PREFIX.length).split(":");
  if (!oid || !isBisectAction(action)) return null;
  return { action, oid };
}

export type BisectAction = "start" | BisectMark | "reset";

function isBisectAction(value: string | undefined): value is BisectAction {
  return value === "start" || value === "good" || value === "bad" || value === "skip" || value === "reset";
}

/** The Bisect submenu of a commit row in the graph: SmartGit's Mark as Good on any commit. */
export function bisectCommitMenu(state: RepoState | null | undefined, oid: string): ContextItem {
  const bisect = bisectOf(state);
  const id = (action: BisectAction) => `${BISECT_MENU_PREFIX}${action}:${oid}`;
  return submenu("bisect", "Bisect", [
    offer(id("start"), "Start…", startBlocked(state)),
    SEPARATOR,
    offer(id("good"), "Mark as Good", markBlocked(bisect, "good", oid)),
    offer(id("bad"), "Mark as Bad", markBlocked(bisect, "bad", oid)),
    offer(id("skip"), "Skip", markBlocked(bisect, "skip", oid)),
    SEPARATOR,
    offer(id("reset"), "Reset", bisect ? null : "no bisect in progress"),
  ]);
}

const capital = (reason: string | null) =>
  reason === null ? undefined : `${reason[0]?.toUpperCase()}${reason.slice(1)}`;

/** Branch ▸ Bisect and the palette: the marks act on HEAD, the commit git checked out. */
export function bisectCommands(
  state: RepoState | null | undefined,
  run: (action: BisectAction) => void,
): PaletteCommand[] {
  const bisect = bisectOf(state);
  const head = bisect?.current ?? null;
  const mark = (id: string, title: string, action: BisectMark): PaletteCommand => ({
    id,
    title,
    synonyms: ["bisect", `git bisect ${action}`],
    unavailable: capital(markBlocked(bisect, action, head)),
    run: () => run(action),
  });
  return [
    {
      id: "bisect-start",
      title: "Start Bisect…",
      synonyms: ["bisect", "find the commit that broke it", "git bisect start"],
      unavailable: capital(startBlocked(state)),
      run: () => run("start"),
    },
    mark("bisect-good", "Bisect: Mark HEAD as Good", "good"),
    mark("bisect-bad", "Bisect: Mark HEAD as Bad", "bad"),
    mark("bisect-skip", "Bisect: Skip HEAD", "skip"),
    {
      id: "bisect-reset",
      title: "Reset Bisect",
      synonyms: ["bisect", "leave bisect", "git bisect reset"],
      unavailable: bisect ? undefined : "No bisect in progress",
      run: () => run("reset"),
    },
  ];
}

/** The Start dialog's fields. From a commit's menu that commit is the good one, as it is
    usually older than the HEAD that shows the problem; from HEAD's menu, HEAD is bad. */
export function startFields(clicked: string | null, head: string | null): { bad: string; good: string } {
  return { bad: "HEAD", good: clicked === null || clicked === head ? "" : clicked };
}

export function startProblem(bad: string, good: string): string | null {
  if (bad.trim() === "") return "Name the bad commit: HEAD, a branch, a tag or an id.";
  if (good.trim() !== "" && good.trim() === bad.trim()) return "The good and the bad commit must differ.";
  return null;
}

/** The first bad commit when this repository has just named it. A repository that only
    comes on screen, already at the end, keeps the selection: the banner can show it. */
export function newlyFound(
  before: { repo: string; firstBad: string | null } | null,
  repo: string,
  bisect: BisectState | null,
): string | null {
  const found = bisect?.firstBad ?? null;
  if (found === null || before?.repo !== repo) return null;
  return before.firstBad === found ? null : found;
}

/** Reset after the end is SmartGit's Leave Bisect and asks nothing; before it, the marks
    made so far are lost. */
export function resetQuestion(
  bisect: BisectState,
): { title: string; message: string; confirm: string; warning: true } | null {
  if (bisectPhase(bisect) === "found") return null;
  return {
    title: "Reset the Bisect",
    message:
      `Reset the bisect? The commits marked so far are forgotten, and ` +
      `${startName(bisect)} is checked out again.`,
    confirm: "Reset",
    warning: true,
  };
}

function startName(bisect: BisectState): string {
  const start = bisect.start.trim();
  if (start === "") return "the commit it began on";
  return /^[0-9a-f]{40}$/i.test(start) ? shortOid(start) : start;
}

/** The banner over the graph: what to do next, and buttons that act on HEAD. A HEAD that
    is marked already has nothing to mark here (M11: the banner hides what does not apply). */
export function bisectBanner(
  bisect: BisectState,
  describe: (oid: string) => string | null = () => null,
): Banner {
  const commit = (oid: string) => {
    const summary = describe(oid);
    return summary ? `${shortOid(oid)} “${summary}”` : shortOid(oid);
  };
  const { bad, good } = bisect.terms;
  const back = `Reset checks out ${startName(bisect)} again.`;
  const head = bisect.current;
  const marked =
    head !== null && (bisect.bad === head || bisect.good.includes(head) || bisect.skipped.includes(head));
  const marks: BannerAction[] = marked ? ["resetBisect"] : ["markGood", "markBad", "markSkip", "resetBisect"];
  const where = marked ? "in the graph" : "in the graph, or HEAD here";
  const going = (detail: string): Banner => ({
    title: "Bisect in progress",
    detail,
    severity: "warning",
    actions: marks,
  });
  switch (bisectPhase(bisect)) {
    case "found":
      return {
        title: "First bad commit found",
        detail: `${commit(bisect.firstBad ?? "")} is the first ${bad} commit. ${back}`,
        severity: "warning",
        actions: ["showFirstBad", "resetBisect"],
      };
    case "stuck":
      return {
        title: "Bisect cannot narrow it down",
        detail:
          `Only skipped commits are left: the first ${bad} commit is one of ` +
          `${bisect.candidates.map(shortOid).join(", ")}. Mark one of them in the graph, or reset.`,
        severity: "warning",
        actions: ["resetBisect"],
      };
    case "testing":
      return going(
        `Test ${commit(bisect.current ?? "")} and mark it; git then checks out the next one. ${back}`,
      );
    case "needGood":
      return going(`Waiting for a ${good} commit, one without the problem: mark it ${where}.`);
    case "needBad":
      return going(`Waiting for a ${bad} commit, one with the problem: mark it ${where}.`);
    case "waiting":
      return going(`Mark a ${bad} commit and a ${good} one ${where}.`);
  }
}
