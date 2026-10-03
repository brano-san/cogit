<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" alt="Cogit" width="128" height="128">
</p>

<h1 align="center">Cogit</h1>

<p align="center">
  Blazing-fast, SmartGit-inspired Git GUI client for power users. Built with Rust, Tauri v2 and Svelte 5.
</p>

<p align="center">
  <a href="https://github.com/brano-san/cogit/actions/workflows/ci.yml"><img src="https://github.com/brano-san/cogit/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/brano-san/cogit/releases"><img src="https://img.shields.io/github/v/release/brano-san/cogit?include_prereleases&sort=semver" alt="Release"></a>
  <a href="LICENSE"><img src="https://img.shields.io/github/license/brano-san/cogit" alt="License: MIT"></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Windows-10%20%7C%2011-brightgreen?logo=windows&logoColor=white" alt="Windows 10 | 11">
  <img src="https://img.shields.io/badge/Ubuntu-22.04%2B-brightgreen?logo=ubuntu&logoColor=white" alt="Ubuntu 22.04+">
  <img src="https://img.shields.io/badge/Debian-12%2B-brightgreen?logo=debian&logoColor=white" alt="Debian 12+">
  <img src="https://img.shields.io/badge/macOS-experimental-yellow?logo=apple&logoColor=white" alt="macOS experimental">
  <img src="https://img.shields.io/badge/Git-2.45%2B-brightgreen?logo=git&logoColor=white" alt="Git 2.45+">
</p>

> **Status: early development.** The application shell runs; Git functionality is being
> built module by module.

## Supported platforms

| Platform | Status | Needs |
|---|---|---|
| Windows 10 / 11 (x64) | installer (`.msi`, `-setup.exe`) and portable `.zip` | Microsoft Edge WebView2 Runtime (preinstalled on Windows 11; the installers fetch it if missing) |
| Ubuntu 22.04+ / Debian 12+ (x86_64) | `.deb` and portable `.AppImage` | `libwebkit2gtk-4.1`, GTK 3, glibc 2.35+ |
| macOS (Apple Silicon) | experimental `.dmg`, unsigned | Gatekeeper asks for a manual open |

Every platform also needs the system `git` (2.45 or newer): reading history works without
it, but every write (commit, checkout, merge, push) runs `git`. Cogit explains how to point
it at your `git` if it is not in `PATH`.

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


## Building

Requires Rust 1.98+, Node 22.12+, Git 2.45+, and a C++ toolchain
(MSVC on Windows; `scripts/wsl/setup-linux.sh` installs everything on Ubuntu 22.04+).

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

## Linux / WSL

Cogit runs on Linux, including WSL2 through WSLg. The window takes its icon from a
launcher entry named `cogit`, so install one of:

- the `.deb` package (`sudo apt install ./Cogit_*_amd64.deb`), which puts
  `cogit.desktop` and the icons under `/usr/share`;
- or, for a portable binary, `cogit --install-desktop-entry`. It writes
  `~/.local/share/applications/cogit.desktop` (honoring `$XDG_DATA_HOME`) and the icons
  under `~/.local/share/icons/hicolor`, pointing at the binary you ran.
  `cogit --uninstall-desktop-entry` removes exactly those files.

Under WSL, run `wsl --shutdown` from Windows after installing so that WSLg reloads its
icons; otherwise the taskbar keeps the generic Linux penguin. Cogit then also appears in
the Windows Start menu, in the folder of the distribution.

### Linux requirements

Cogit needs glibc 2.35 or newer (the release is built on Ubuntu 22.04) and these runtime
libraries. A missing one is reported by the dynamic loader before Cogit starts (for example
`libwebkit2gtk-4.1.so.0: cannot open shared object file`); Cogit cannot show its own message
for that, so install them first:

```sh
# Debian / Ubuntu
sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0 libayatana-appindicator3-1 librsvg2-2 \
  libsoup-3.0-0 libjavascriptcoregtk-4.1-0 libssl3 libdbus-1-3 libsecret-1-0 libxdo3 git
# Fedora
sudo dnf install webkit2gtk4.1 gtk3 libappindicator-gtk3 librsvg2 libsoup3 openssl \
  dbus-libs libsecret xdotool git
# Arch
sudo pacman -S webkit2gtk-4.1 gtk3 libayatana-appindicator librsvg libsoup3 openssl \
  libsecret xdotool git
```

The `.deb` pulls these in itself. Without `git`, Cogit starts and offers to find or install
one. On Windows a missing WebView2 Runtime is reported in a message box with the download
link.

`cogit --version` and `cogit --help` print and exit without opening a window.

## Portable build

The Windows `Cogit_<version>_x64_portable.zip` and the Linux `.AppImage` are a separate
build of the same code (cargo feature `portable`; `npm run tauri build -- --no-bundle
--features portable`). There is no marker file and no switch at run time: the binary knows
it is portable.

- Everything Cogit writes lives in a `Cogit-data` folder beside the binary (beside the
  `.AppImage` file for an AppImage): `config/` (settings, window state), `local/` (webview
  profile), `logs/`, `tmp/`, `cache/`. Keep it on a writable disk: if the folder cannot be
  written, Cogit exits with an error on stderr instead of writing anywhere else.
- It never updates itself; replace the file to update. Settings stay in `Cogit-data`.
- git, ssh and credential helpers see your normal environment. HTTP tokens still go to the
  system keyring (Credential Manager / Secret Service). A few writes outside the folder
  are not Cogit's to control (the WebView2 runtime, the Windows jump list and
  `Zone.Identifier`).

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
```

## Contributing

Read [`CLAUDE.md`](CLAUDE.md) first — it holds the architectural rules, the error-handling
policy and the testing approach. 

## License

MIT. See [LICENSE](LICENSE).
