# Rust crates

## Style

- `thiserror` in crates, `anyhow` only at entry points.
- No `.unwrap()` / `.expect()` on dynamic Git operations (Clippy denies; `clippy.toml`
  allows them in tests). Detached HEAD, empty repo, locked index, missing refs → typed
  errors.
- Normalize line endings to `\n` before diffing, honoring `.gitattributes` and
  `core.autocrlf`; keep the original ending.
- IPC paths: `/` on every platform, relative to the repository root. OIDs:
  `gix::ObjectId` in Rust, hex strings over IPC.
- Comments only for non-obvious edge cases, trade-offs and safety invariants, under 5% of
  a file's lines. One-line docstrings, only where the name does not say it.
  Check: `cm=$(grep -cE '^[[:space:]]*(//|/*|*)' F); tot=$(grep -c '' F); echo $((cm*100/tot))%`

## Logging

- `tracing` only; `println!` / `eprintln!` are denied.
- Every fallback logs its `Err`: `tracing::error!(error = ?err, context = "…")`.
- Boundaries and `elapsed` at `info` for heavy work (graph build, large diff, fetch) and
  for anything that can hang (open, close, switch repository; spawn window; shutdown).
  Never inside tight loops.
- One file per run `cogit-<start>.log`, `….2.log` past 10 MB, at most 10 files,
  non-blocking. Keep `WorkerGuard` alive for the whole process or the tail is lost.
  Windows folder: `%LOCALAPPDATA%\dev.branosan.cogit\logs\`.

## Git

- Spawning `git`: `GIT_TERMINAL_PROMPT=0`, `current_dir(repo_root)`, args as an array,
  `LC_ALL=C` when parsing output.
- Paths inside `.git` via `rev-parse --git-path` or its gix equivalent, never hand-built.
- Peel refs to a commit (`^{commit}`) before walking; skip tags not pointing at a commit.
- A nested submodule's path is relative to its direct parent; a gitlink is mode `160000`.
- Published = reachable from any remote-tracking branch (`is_published`); unknown or timed
  out counts as published.
- Never change the user's repository without explicit confirmation.

## Tests

- Strict TDD: the failing test first.
- Algorithms (graph layout, filtering, sorting) are pure functions with unit tests.
- Fixtures: system git with `HOME` and `GIT_CONFIG_GLOBAL` redirected; deterministic OIDs.
  Prefer `fast-import` (R-56) — it writes no reflog and merges no trees, arrange those
  yourself.
- Tests that assert durations flake under load, so hooks run none: run touched targets
  while working, the full suite once before handing over.