# Cogit

Git client: Rust workspace + Tauri v2 + Svelte 5. `crates/`, `frontend/` have own CLAUDE.md.

## Docs (`doc/`, RU) — read before coding

Always: `00-roadmap.md` (modules, order, status), `01-architecture.md` (crate boundaries, INV-01…12). As needed: `modules/M<N>-*.md` module N; `02-tech-stack` deps; `03-git-semantics` any Git op; `04-ipc-contract` commands/events/DTOs; `07-graph-rendering` graph; `08-diff-engine` diff, patch staging, 3-way merge; `11-keybindings`; `12-risks` spec deviations (record here); `13-distribution` releases; `14-profiling`, `15-benchmark` speed; `features/` one file per shipped feature.
Breaking an invariant → edit `01-architecture.md` with justification, no local exceptions.

## APIs

gix, imara-diff, keyring, similar, Svelte 5, Tauri v2 broke APIs in 2025–26; training data is stale. Check docs.rs for the `Cargo.lock` version first. imara-diff 0.2 ≠ 0.1.

## Task lists (RU, numbered/checklist, from manual testing)

- Reproduce first; may be fixed already.
- Recurring item → root cause, don't stack fixes.
- No screenshots; text describes them.
- Keep numbering. Tick `[x]` only after checking in a running build, add `> Итог: <cause> → <change>` | `уже работало` | `не сделано: <reason>`.
- «На твоё решение»/«обоснуй» → one line of reasoning in report; spec deviation also → `12-risks.md`.
- One commit per item where practical; map items→commits in report; never `#N` in messages.
- Report: short, Russian, no recap.

## Architecture

- Logic in `crates/*`; `src-tauri` = IPC routing, windows, plugins; command body ~10 lines.
- Crates never depend on `tauri` (`specta` derives only).
- Reads: gix. Writes: system `git`, spawned only by `git_engine`. Table: `03-git-semantics.md`.
- Heavy work → `spawn_blocking`/rayon; no rayon inside async; no `parking_lot` guard across `await`.
- >~500 items → `tauri::ipc::Channel`, chunks 100–200.
- Highlighting: Lezer/CodeMirror; tree-sitter only for AST diff, syntactic merge.
- Graph lines: Canvas under virtualized list, not SVG.

## Performance

- UI never blocked >50 ms.
- One op queue per repo: writes ordered, reads parallel + cancellable, nothing dropped.
- Send only visible; virtualize; dedupe refresh cascades.
- Speed change: benchmark before/after; no gain → revert.

## Graph (authoritative; sync `07-graph-rendering.md` in same commit)

- Lane 0 = first-parent chain of HEAD (else `master`, then `main`): continuous, fixed. Others compact; new lane right next to its commit's lane.
- Lane changes = S-curves within one row; lines meet node centers.
- Monochrome default: main line bright, rest gray. Branches ticked in Branches get own color from tip down first parents to join point; history already on main line keeps default.
- Color/emphasis = paint over finished layout (`graph_engine::paint`); never alters layout.

## Errors

- Never truncate/mask/replace raw git output, except redacting credentials in URLs/auth headers.
- Non-zero exit → `GitCommandError` (command, exit code, full stdout+stderr). Capture both on success too (push writes stderr).
- One notification queue: errors first, nothing dropped, closing shows next. Footer `Error` clears when none left.
- Normal repo states (uninit submodule, commit missing locally, detached HEAD in submodule) ≠ `Internal error`: explain, offer fix.

## Windows

- Child windows (diff, blame, investigate): fill client area, no main menu, close via button/`Esc`/`Ctrl+W`, theme bg, never affect main window.
- Geometry validated once at startup; not saved while max/minimized; never moved on own move events. Wayland can't set position.
- Platform-specific actions (terminal, file manager, PowerShell, Git Shell, `Ctrl`/`⌘`) via one abstraction.

## Commits

Conventional Commits, scope = module: `fix(m1): handle a locked index without panicking`. One line, imperative, ≤72 chars, no period/body/trailers. Reasoning → `doc/`. Propose name at end of each step, before committing.

## Before committing

- Once per clone: `git config core.hooksPath .githooks` (fmt, clippy, bindings, svelte-check; no tests).
- Same commit: module status in `00-roadmap.md`; user-visible feature → `features/F-NNN-<slug>.md` + row in `features/README.md`; deviations → `12-risks.md`; IPC → `04-ipc-contract.md`; shortcuts → `11-keybindings.md`.
- Before handover: full suite once, idle machine.

## Commands

```bash
cargo check -p <crate>                         # fastest
cargo nextest run -p <crate> --test <name>     # touched targets
cargo nextest run --workspace --exclude cogit  # full suite
cargo test -p cogit --lib                      # nextest lacks tauri manifest
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo deny check
cargo insta review                             # never accept blindly
cargo run -p cogit --bin export-bindings       # after command/DTO change
npm run tauri dev | npm run tauri build
npm --prefix frontend run check | test
npx vitest run <path>
```
Once: `cargo install cargo-nextest --locked`.

## Language

English: code, comments, commits, UI, CLAUDE.md. Russian: `doc/`, reports. User-facing text (frontend, native menu, Rust errors/journal summaries) uses US spelling (`initialize`, `color`, `license`, `canceled`); serde names, setting keys, command ids unchanged.

## Rules & Constraints
- Think, reason, and respond strictly in English.
- Be concise and direct to save tokens: avoid pleasantries, verbose explanations, and conversational filler.