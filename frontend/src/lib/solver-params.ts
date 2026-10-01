import type { RepoId } from "./ipc";

export interface SolverRequest {
  repo: RepoId;
  path: string;
  /** Start the external merge tool as soon as the file is read. */
  tool: boolean;
}

/** Built by `solver_window::url` in Rust; read once, when the window starts. */
export function parseSolver(search: string): SolverRequest | null {
  const params = new URLSearchParams(search);
  const repo = Number(params.get("repo"));
  const path = params.get("path");
  if (!Number.isInteger(repo) || repo <= 0 || !path) return null;
  return { repo, path, tool: params.get("tool") === "1" };
}
