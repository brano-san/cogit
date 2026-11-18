import type { Branch, CiCheck, CiState, CiStatus, CiTarget, ContextItem } from "$lib/ipc";
import { SEPARATOR, item, tidy } from "$lib/context-menu";

export const GRAPH_LIMIT = 80;
export const BRANCH_LIMIT = 60;
export const MIN_POLL_MS = 3_000;
export const MAX_POLL_MS = 600_000;
export const MENU_PREFIX = "ci-open:";

export function graphTargets(oids: readonly string[], unpushed: (oid: string) => boolean, limit = GRAPH_LIMIT): CiTarget[] {
  const seen = new Set<string>();
  const targets: CiTarget[] = [];
  for (const sha of oids) {
    if (targets.length >= limit) break;
    if (seen.has(sha) || unpushed(sha)) continue;
    seen.add(sha);
    targets.push({ sha, remote: null });
  }
  return targets;
}

export function remoteOf(name: string, remotes: readonly string[]): string | null {
  let found: string | null = null;
  for (const remote of remotes) {
    if (name.startsWith(`${remote}/`) && (found === null || remote.length > found.length)) found = remote;
  }
  return found;
}

export function branchTargets(branches: readonly Branch[], remotes: readonly string[], limit = BRANCH_LIMIT): CiTarget[] {
  const seen = new Set<string>();
  const targets: CiTarget[] = [];
  for (const branch of branches) {
    if (targets.length >= limit) break;
    const tracked = branch.kind === "local" ? branch.upstream : branch.kind === "remote" ? branch.name : null;
    if (!tracked) continue;
    if (branch.kind === "local" && branch.ahead > 0) continue;
    const remote = remoteOf(tracked, remotes);
    if (remote === null || seen.has(branch.oid)) continue;
    seen.add(branch.oid);
    targets.push({ sha: branch.oid, remote });
  }
  return targets;
}

export function mergeTargets(...lists: readonly (readonly CiTarget[])[]): CiTarget[] {
  const seen = new Set<string>();
  const merged: CiTarget[] = [];
  for (const list of lists) {
    for (const target of list) {
      if (seen.has(target.sha)) continue;
      seen.add(target.sha);
      merged.push(target);
    }
  }
  return merged;
}

export function sameTargets(a: readonly CiTarget[], b: readonly CiTarget[]): boolean {
  return a.length === b.length && a.every((target, at) => target.sha === b[at]!.sha && target.remote === b[at]!.remote);
}

export function nextPoll(retryMs: number | null): number | null {
  if (retryMs === null) return null;
  return Math.min(MAX_POLL_MS, Math.max(MIN_POLL_MS, retryMs));
}

export function shown(status: CiStatus | undefined): status is CiStatus {
  return status !== undefined && status.state !== "none";
}

const WORD: Record<CiState, string> = {
  success: "passed",
  failure: "failed",
  pending: "running",
  neutral: "skipped",
  none: "no checks",
};

const MARK: Record<CiState, string> = {
  success: "✓",
  failure: "✗",
  pending: "●",
  neutral: "–",
  none: "",
};

export function durationText(secs: number): string {
  if (secs < 60) return `${secs} s`;
  const minutes = Math.floor(secs / 60);
  if (minutes < 60) return secs % 60 === 0 ? `${minutes} min` : `${minutes} min ${secs % 60} s`;
  const hours = Math.floor(minutes / 60);
  return minutes % 60 === 0 ? `${hours} h` : `${hours} h ${minutes % 60} min`;
}

export function checkLine(check: CiCheck): string {
  const duration = check.durationSecs === null ? "" : `, ${durationText(check.durationSecs)}`;
  return `${MARK[check.state]} ${check.name} — ${WORD[check.state]}${duration}`;
}

export function ciSummary(status: CiStatus): string {
  const count = status.checks.length;
  return `CI ${WORD[status.state]}: ${count} ${count === 1 ? "check" : "checks"}`;
}

export function ciTooltip(status: CiStatus): string {
  const lines = [ciSummary(status), ...status.checks.map(checkLine)];
  if (status.note) lines.push(status.note);
  lines.push("Click for links");
  return lines.join("\n");
}

export interface CiMenu {
  items: ContextItem[];
  urls: (string | null)[];
}

export function ciMenu(status: CiStatus): CiMenu {
  const urls: (string | null)[] = [];
  const items: ContextItem[] = [];
  if (status.note) items.push(item(`${MENU_PREFIX}note`, status.note, false), SEPARATOR);
  for (const check of status.checks) {
    urls.push(check.url);
    items.push(item(`${MENU_PREFIX}${urls.length - 1}`, checkLine(check), check.url !== null));
  }
  urls.push(status.url);
  items.push(SEPARATOR, item(`${MENU_PREFIX}${urls.length - 1}`, "Open All Checks in Browser", status.url !== null));
  return { items: tidy(items), urls };
}

export function menuUrl(menu: CiMenu | null, id: string): string | null | undefined {
  if (!id.startsWith(MENU_PREFIX)) return undefined;
  const at = Number(id.slice(MENU_PREFIX.length));
  if (!menu || !Number.isInteger(at)) return null;
  return menu.urls[at] ?? null;
}
