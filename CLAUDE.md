# Cogit

Desktop Git client: Rust workspace + Tauri v2 + Svelte 5. `crates/` and `frontend/` have
their own CLAUDE.md with area rules.

## Docs (`doc/`, Russian) — read before writing code

| File | Read when |
|---|---|
| `00-roadmap.md` | always first: modules, order, status |
| `01-architecture.md` | always: crate boundaries, invariants INV-01…INV-12 |
| `modules/M<N>-*.md` | working on module N |
| `02-tech-stack.md` | touching a dependency |
| `03-git-semantics.md` | any Git operation |
| `04-ipc-contract.md` | changing a command, event or DTO |
| `07-graph-rendering.md` | graph |
| `08-diff-engine.md` | diff, staging patches, 3-way merge |
| `11-keybindings.md` | shortcuts |
| `12-risks.md` | deviating from the spec — record the decision here |
| `13-distribution.md` | release builds, installers |
| `14-profiling.md`, `15-benchmark.md` | speed: profile log; end-to-end benchmark and budgets |
| `features/` | one file per shipped user-visible feature |

Breaking an invariant means editing `01-architecture.md` with a justification, never a
local exception.

## Verify APIs first

gix, imara-diff, keyring, similar, Svelte 5 and Tauri v2 all broke their APIs in
2025–2026, and training data shows the old ones. Before using an unfamiliar API, open
docs.rs for the exact version in `Cargo.lock`. imara-diff 0.2 shares almost nothing
with 0.1.

## Task lists

Tasks arrive as a short numbered list or a Markdown checklist in Russian, written after
manual testing. This file applies to them; the task text does not repeat it.

- Reproduce first — the item may already be fixed.
- An item that returns after an earlier fix: find the root cause, do not stack fixes.
- Screenshots are not available; the task text describes them.
- Keep the task's item numbers. Tick `[x]` only after checking in a running build and add
  one line: `> Итог: <cause> → <change>`, `уже работало` or `не сделано: <reason>`.
- «На твоё решение» / «обоснуй» → one line of reasoning in the report; a deviation from
  the spec also goes to `12-risks.md`.
- One commit per item where practical; map items to commits in the report, never `#N`
  in a message (it links an unrelated issue).
- Report: short, Russian, no recap.

## Architecture

- Business logic lives in `crates/*`. `src-tauri` routes IPC, owns windows, wires plugins;
  a command body is about ten lines.
- Crates never depend on `tauri` (`specta` derives only).
- Reads use gix; writes use the system `git`, which already handles hooks, credentials and
  merge strategies. Only `git_engine` spawns `git`. Per-operation table:
  `03-git-semantics.md`.
- Never block: heavy work goes to `spawn_blocking` or rayon, never rayon inside an async
  task. No `parking_lot` guard across `await`.
- Over ~500 items: `tauri::ipc::Channel`, chunks of 100–200.
- Syntax highlighting: Lezer in CodeMirror on the frontend. tree-sitter only for AST diff
  and syntactic merge.
- Graph lines: Canvas under a virtualized list, not SVG.

## Performance

- Nothing blocks the UI for more than 50 ms.
- One operation queue per repository: writes in order, reads parallel and cancellable,
  nothing dropped.
- Send only what is visible; virtualize lists; deduplicate refresh cascades.
- A speed change is measured with the benchmark before and after; no gain → revert.

## Graph

- Lane 0 is the first-parent chain of HEAD (else `master`, then `main`): continuous, never
  moves. Other lanes compact; a new lane goes right next to its commit's lane.
- Lane changes are S-curves within one row; lines meet nodes at the centre.
- Monochrome by default: main line bright, the rest grey; branches ticked in Branches
  in their own colours, from the tip down its first parents to the line it joins; history
  the main line already has, merged or not, keeps its default colour.
- Colour and emphasis are paint over the finished layout (`graph_engine::paint`); the
  layout itself never changes for them.
- This section is current. If `07-graph-rendering.md` disagrees, bring it in line in the
  same commit.

## Errors

- Raw git output is never truncated, masked or replaced. Only exception: credentials in
  URLs and auth headers are redacted.
- Non-zero exit → `GitCommandError` with the command, exit code and both streams in full.
  Capture both streams on success too — a successful push writes to stderr.
- One notification queue for errors and warnings: errors first, nothing dropped, closing
  an entry shows the next. Footer `Error` clears when no errors remain.
- Normal repository states (uninitialised submodule, commit missing locally, detached
  HEAD in a submodule) are not `Internal error`: say what happened, offer the fix.

## Windows

- Child windows (diff, blame, investigate): fill the client area, no main-window menu,
  close on the button, `Esc` and `Ctrl+W`, theme background. They never affect the main
  window.
- Geometry is validated once, at startup. Never saved while maximized or minimized; never
  moved in response to its own move events. Wayland cannot set a position.
- Platform-specific actions (terminal, file manager, PowerShell, Git Shell, `Ctrl`/`⌘`)
  go through one abstraction.

## Commits

Conventional Commits, scope = roadmap module: `fix(m1): handle a locked index without
panicking`. One line, imperative, ≤72 chars, no period, no body, no trailers. Reasoning
goes to `doc/`. Propose the name at the end of each step, before committing.

## Before committing

- Once per clone: `git config core.hooksPath .githooks` (fmt, clippy, IPC bindings,
  svelte-check — no tests).
- In the same commit: module status in `00-roadmap.md`; `features/F-NNN-<slug>.md` plus
  its row in `features/README.md` for a user-visible feature; spec deviations in
  `12-risks.md`; IPC changes in `04-ipc-contract.md`; shortcuts in `11-keybindings.md`.
- Before handing work over: the full suite, once, on an idle machine.

## Commands

```bash
cargo check -p <crate>                          # fastest feedback
cargo nextest run -p <crate> --test <name>      # while working: touched targets only
cargo nextest run --workspace --exclude cogit   # full suite
cargo test -p cogit --lib                       # tauri needs a manifest nextest lacks
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo deny check
cargo insta review                              # never accept snapshots blindly
cargo run -p cogit --bin export-bindings        # after changing a command or DTO
npm run tauri dev
npm run tauri build
npm --prefix frontend run check
npm --prefix frontend run test
npx vitest run <path>
```

Once: `cargo install cargo-nextest --locked`.

## Language

Code, comments, commits, UI strings, CLAUDE.md: English. `doc/` and task reports: Russian.