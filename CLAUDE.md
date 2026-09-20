# Cogit — Development Guidelines

Cogit is a desktop Git client: Rust workspace + Tauri v2 + Svelte 5.

**Planning docs live in `doc/` (Russian).** Read them before writing code:

| Read this | When |
|---|---|
| `doc/00-roadmap.md` | Always first — modules, order, status |
| `doc/01-architecture.md` | Always — crate boundaries and the invariant registry |
| `doc/modules/M<N>-*.md` | Before working on a module |
| `doc/02-tech-stack.md` | Before touching any dependency |
| `doc/03-git-semantics.md` | Any Git operation |
| `doc/04-ipc-contract.md` | Any change to a Tauri command, event or DTO |
| `doc/07-graph-rendering.md` | Commit graph work |
| `doc/08-diff-engine.md` | Diff, staging patches, 3-way merge |
| `doc/12-risks.md` | Before deviating from the spec — record the decision there |
| `doc/features/` | One file per user-visible feature, one sentence each — add one whenever you ship a feature |
| `doc/13-distribution.md` | Release builds, installers, what the end user's machine needs |

## Verify APIs before writing code

`gix`, `imara-diff`, `keyring`, `similar`, Svelte 5 and Tauri v2 all shipped breaking
changes during 2025–2026. Training data is full of the old APIs.

**Before using an unfamiliar API, open docs.rs for the exact version in `Cargo.lock`.**
This is not optional. `imara-diff` 0.2 in particular shares almost nothing with the 0.1
examples that appear everywhere online.

## Architecture rules

- **Modular workspace.** All business logic belongs in `crates/*`. `src-tauri` only routes
  IPC, owns windows and wires plugins. A command body is roughly ten lines: fetch state,
  call a crate, map the error.
- **Crates never depend on `tauri`.** They may depend on `specta` for derive macros only.
  This keeps `cargo test -p <crate>` fast and GUI-free.
- **Never block threads.** Heavy Git and diff work goes in `tokio::task::spawn_blocking`
  or `rayon`. Never call `rayon` directly inside an async task — it blocks a Tokio worker
  and stalls the whole runtime, IPC included.
- **Never hold a `parking_lot` guard across an `await`.**
- **Stream large results.** Anything over ~500 items goes through `tauri::ipc::Channel`
  in chunks of 100–200, never as one JSON return value.
- **Syntax highlighting is a frontend concern.** Lezer inside CodeMirror does it.
  Do not serialize tree-sitter tokens from Rust. `tree-sitter` is for AST diffs and
  syntactic merge only.
- **Graph lines render on HTML5 Canvas**, under a virtualized HTML list. Not SVG nodes.

The full invariant registry (INV-01 … INV-12) is in `doc/01-architecture.md`. Violating
one means editing that document with a justification — not making a local exception.

## Comments

Write clean, self-documenting code. Do NOT write trivial or obvious comments explaining
what the code does line by line. Add comments ONLY for non-obvious edge cases,
architectural trade-offs, or safety invariants.

**Hard ceiling: comments must stay under 5% of a file's lines.** Docstrings are one line
and only where the name does not already say it. Reasoning belongs in `doc/`, not here.

```sh
cm=$(grep -cE '^[[:space:]]*(//|/*|*)' FILE); tot=$(grep -c '' FILE); echo $((cm*100/tot))%
```

## Code style and safety

- `thiserror` inside crates, `anyhow` only at application entry points.
- **No `.unwrap()` or `.expect()` on dynamic Git operations.** Clippy denies them
  workspace-wide; `clippy.toml` allows them in tests. Handle detached HEAD, empty
  repositories, a locked index and missing refs through typed errors.
- Always normalize line endings to `\n` before diffing, honouring `.gitattributes`
  and `core.autocrlf`. Store the original ending separately.
- Paths crossing IPC use `/` on every platform and are relative to the repository root.
- Object IDs are `gix::ObjectId` in Rust and hex strings over IPC.

## Logging

- `tracing` only. `println!` and `eprintln!` are denied by Clippy.
- Log the `Err` branch of every fallback with structured context:
  `tracing::error!(error = ?err, context = "failed to resolve ref")`.
- Log the boundaries and `elapsed` of heavy operations (graph build, large diff, fetch).
- Do not log inside tight loops. Log the outcome, not the iterations.
- The log file is capped at 10 MB with 2 archives, written non-blocking. The
  `WorkerGuard` must stay alive for the process lifetime or the tail of the log is lost.

## Git CLI error handling

- **Never truncate, mask or replace raw Git CLI output.** `stderr` is where the user finds
  the pull-request link, the pre-receive hook message and the reason a push was rejected.
- A non-zero exit code produces a structured `GitCommandError` carrying the command,
  the exit code and both streams in full.
- The frontend shows those raw streams in the Git Error Dialog, with clickable URLs and
  a "Copy Output" action.
- Capture `stdout` and `stderr` even on success — a successful `push` writes to `stderr`.
- Always set `GIT_TERMINAL_PROMPT=0`, `current_dir(repo_root)`, and pass arguments as an
  array. Set `LC_ALL=C` whenever the output is parsed.

## Reads use gix, writes use the git CLI

Reading goes through `gix` because spawning a process costs 5–30 ms on Windows and the
graph needs tens of thousands of objects. Writing goes through the system `git` because
it already handles hooks, credentials, rebase and merge strategies correctly, and a bug
in our write path costs the user their work.

`doc/03-git-semantics.md` has the per-operation table. Nothing outside `git_engine`
may spawn a `git` process.

## Testing

Strict TDD for Rust crates — write the failing test first. Pragmatic for UI: test store
logic, virtualization and geometry; do not test markup.

```bash
cargo check -p git_engine            # fastest feedback
cargo test -p diff_engine
cargo test -p graph_engine
cargo insta review                   # inspect snapshot changes, never accept blindly
```

Avoid full-workspace rebuilds during micro-iterations. Full `cargo test --workspace`
before committing.

Test fixtures generate temporary repositories with the **system git**, with `HOME` and
`GIT_CONFIG_GLOBAL` redirected so the developer's own `.gitconfig` cannot skew results.
Commit OIDs must be deterministic — snapshot tests depend on it.

## Commit messages

Conventional Commits, English, with the roadmap module as the scope so history
lines up with `doc/00-roadmap.md`:

```
feat(m9): add diamond, octopus and conflict fixtures
fix(m1): handle a locked index without panicking
test(m4): snapshot lane allocation for octopus merges
docs: record the ComCtl32 manifest finding
chore: pin gix to 0.87 and enable the status feature
refactor(m7): split hunk assembly out of the diff pipeline
```

**One line only. No body, no trailers** — no `Co-Authored-By`, no `Signed-off-by`.
Imperative, under ~72 characters, no trailing period. Reasoning belongs in `doc/`,
not in the commit log — `doc/12-risks.md` records every decision taken against the
original spec.

Propose the commit name at the end of each step of work, before committing.

## Before committing

The pre-commit hook runs fmt, clippy, tests and `svelte-check`. Enable it once per clone:

```
git config core.hooksPath .githooks
```

Then, by hand:
- Update module status in `doc/00-roadmap.md` (the single source of truth for status).
- Add `doc/features/F-NNN-<slug>.md` for every user-visible feature: a title line and one
  sentence someone can check by hand. Add its row to `doc/features/README.md`.
- Record any decision taken against the spec in `doc/12-risks.md`.
- Reflect IPC changes in `doc/04-ipc-contract.md` and shortcuts in `doc/11-keybindings.md`,
  in the same commit.

## Commands

```bash
# Rust
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo deny check

# Regenerate frontend IPC types after changing a command or DTO.
# A binary, not a test: linking tauri needs the ComCtl32 v6 manifest that
# tauri-build only embeds into binary targets, so a test harness cannot start.
cargo run -p cogit --bin export-bindings

# App (from repo root)
npm run tauri dev
npm run tauri build

# Frontend only
npm --prefix frontend run check
npm --prefix frontend run test
```

## Language

Code, comments, commit messages, UI strings and this file: **English**.
Planning documents in `doc/`: **Russian**.
