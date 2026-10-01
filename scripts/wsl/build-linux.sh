#!/usr/bin/env bash
# Builds and checks Cogit in a WSL clone at ~/cogit, synced from the Windows checkout.
#
#   scripts/wsl/build-linux.sh [WINDOWS_REPO]        sync, frontend, cargo check, debug build
#   scripts/wsl/build-linux.sh [WINDOWS_REPO] tests  also the nextest suite (long)
#   scripts/wsl/build-linux.sh [WINDOWS_REPO] bundle also a release build with .deb and AppImage
#
# The clone is reset to the Windows repo's HEAD each run: do not edit files in ~/cogit.
set -euo pipefail

SRC="${1:-/mnt/d/cogit}"
STEP="${2:-}"
DEST="$HOME/cogit"

export PATH="$HOME/.cargo/bin:$HOME/.local/node/bin:$PATH"
export RUSTC_WRAPPER=""
export CARGO_TERM_COLOR=always

say() { printf '\033[36m[wsl-build]\033[0m %s\n' "$1"; }

for tool in cc clang mold pkg-config node npm cargo rustup; do
  command -v "$tool" >/dev/null || { echo "missing: $tool (run scripts/wsl/setup-linux.sh)" >&2; exit 1; }
done
pkg-config --exists webkit2gtk-4.1 || { echo "missing: libwebkit2gtk-4.1-dev (run scripts/wsl/setup-linux.sh --system)" >&2; exit 1; }

git config --global --add safe.directory "$SRC" 2>/dev/null || true
git config --global --add safe.directory "$SRC/.git" 2>/dev/null || true

if [ ! -d "$DEST/.git" ]; then
  say "clone $SRC -> $DEST"
  git clone -q "$SRC" "$DEST"
fi
cd "$DEST"
say "sync to $(git -C "$SRC" rev-parse --short HEAD)"
git fetch -q "$SRC" HEAD
git reset -q --hard FETCH_HEAD
git clean -qfd -e target -e node_modules -e frontend/node_modules -e frontend/dist

say "toolchain: $(rustc --version 2>/dev/null || echo 'installing from rust-toolchain.toml')"
rustup show active-toolchain >/dev/null

say "npm ci"
npm ci --no-audit --no-fund

say "frontend: check, colors, build"
npm --prefix frontend run check
node scripts/check-colors.mjs
npm run build

say "cargo check --workspace --all-targets"
cargo check --workspace --all-targets

say "cargo build -p cogit"
cargo build -p cogit

if [ "$STEP" = tests ]; then
  say "nextest (workspace, without the Tauri crate)"
  cargo nextest run --workspace --exclude cogit
  say "cogit lib tests"
  cargo test -p cogit --lib
  say "frontend tests"
  npm --prefix frontend run test
fi

if [ "$STEP" = bundle ]; then
  say "release bundle (deb, appimage)"
  npm run tauri build -- --ci --bundles deb,appimage --config '{"bundle":{"createUpdaterArtifacts":false}}'
fi

say "done"
