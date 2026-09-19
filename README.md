# Cogit

Blazing-fast, SmartGit-inspired Git GUI client for power users. Built with Rust, Tauri v2 and Svelte 5.

> **Status: early development.** The application shell runs; Git functionality is being
> built module by module. See [`doc/00-roadmap.md`](doc/00-roadmap.md) for the plan.

## What it aims to be

A dense, keyboard-driven desktop client that shows five panels at once — repositories,
references, commit graph, changed files and a side-by-side diff — and never hides what
Git actually said.

Three commitments shape every design decision:

- **Speed on real repositories.** 50 000 commits must scroll at 60 FPS. History streams
  in chunks, graph lines render on canvas, and reads go through
  [`gix`](https://github.com/GitoxideLabs/gitoxide) rather than spawning processes.
- **No masked errors.** When a Git command fails you see the command, the exit code and
  the complete `stdout` and `stderr` — including the pull-request link and the
  pre-receive hook message.
- **Nothing destructive without a way back.** Every history-rewriting or working-tree
  operation is recorded before it runs, and can be undone.

## Architecture in one paragraph

Business logic lives in `crates/` as six independent libraries that know nothing about
Tauri, so they test in seconds without a GUI. `src-tauri/` is a thin routing layer.
`frontend/` is a Svelte 5 SPA. Reads use `gix`; writes go through the system `git`, which
already handles hooks, credentials and merge strategies correctly.

Details: [`doc/01-architecture.md`](doc/01-architecture.md).

## Building

Requires Rust 1.98+, Node 22.12+, Git 2.40+, and a C++ toolchain
(MSVC on Windows). Full setup including LLVM and sccache:
[`doc/10-toolchain-setup.md`](doc/10-toolchain-setup.md).

```bash
npm install
npm run tauri dev        # development, with frontend hot reload
npm run tauri build      # release build with installers
```

Working on the Rust side alone is much faster:

```bash
cargo check -p git_engine
cargo test --workspace
```

## Repository layout

```
crates/          business logic, no Tauri dependency
  git_engine/      gix reads + git CLI mutations
  diff_engine/     imara-diff blocks + similar word diff + tree-sitter merge
  graph_engine/    commit DAG topology and lane allocation
  fs_watcher/      debounced, filtered filesystem watching
  app_state/       open repositories, event bus, credentials
  test_fixtures/   generated temporary repositories for tests
src-tauri/       IPC routing, windows, plugins
frontend/        Svelte 5 + Vite + CodeMirror 6
doc/             specifications and module plans (Russian)
```

## Contributing

Read [`CLAUDE.md`](CLAUDE.md) first — it holds the architectural rules, the error-handling
policy and the testing approach. The invariants it references are defined in
[`doc/01-architecture.md`](doc/01-architecture.md).

## License

MIT. See [LICENSE](LICENSE).
