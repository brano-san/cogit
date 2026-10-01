#!/usr/bin/env bash
# Sets up Ubuntu 24.04 (WSL2 or native) to build Cogit. Idempotent.
#
#   scripts/wsl/setup-linux.sh            system packages (asks for sudo) + user tools
#   scripts/wsl/setup-linux.sh --system   only the apt part
#   scripts/wsl/setup-linux.sh --user     only rustup, nextest, Node (no root needed)
#
# Why the clone lives in ~/cogit and not under /mnt/d: a build on the Windows drive through
# 9p is several times slower, and target/ and node_modules/ built on Windows are useless here.
set -euo pipefail

mode="${1:-all}"
NODE_MAJOR=22

say() { printf '\033[36m[wsl-setup]\033[0m %s\n' "$1"; }

system_part() {
  say "apt packages (Tauri v2 prerequisites + linker + keyring/dbus headers)"
  sudo apt-get update
  sudo apt-get install -y --no-install-recommends \
    build-essential pkg-config curl wget file git ca-certificates xz-utils unzip \
    clang mold patchelf \
    libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libsoup-3.0-dev libgtk-3-dev \
    libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev \
    libdbus-1-dev libsecret-1-dev
}

user_part() {
  mkdir -p "$HOME/.local/bin"

  if ! command -v rustup >/dev/null 2>&1 && [ ! -x "$HOME/.cargo/bin/rustup" ]; then
    say "rustup"
    curl -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain none --profile minimal
  fi
  export PATH="$HOME/.cargo/bin:$PATH"

  if [ ! -x "$HOME/.cargo/bin/cargo-nextest" ]; then
    say "cargo-nextest (prebuilt)"
    curl -LsSf https://get.nexte.st/latest/linux | tar zxf - -C "$HOME/.cargo/bin"
  fi

  if [ ! -x "$HOME/.local/node/bin/node" ] || ! "$HOME/.local/node/bin/node" -v | grep -q "^v${NODE_MAJOR}\."; then
    say "Node.js ${NODE_MAJOR} (official tarball, ~/.local/node)"
    base="https://nodejs.org/dist/latest-v${NODE_MAJOR}.x"
    file=$(curl -sSf "$base/SHASUMS256.txt" | awk '/linux-x64.tar.xz$/ {print $2}')
    tmp=$(mktemp -d)
    curl -sSf -o "$tmp/$file" "$base/$file"
    curl -sSf "$base/SHASUMS256.txt" | grep " $file\$" > "$tmp/sum"
    (cd "$tmp" && sha256sum -c sum)
    rm -rf "$HOME/.local/node"
    mkdir -p "$HOME/.local/node"
    tar -xJf "$tmp/$file" -C "$HOME/.local/node" --strip-components=1
    rm -rf "$tmp"
  fi

  # Linux tools must win over the Windows ones WSL appends to PATH (/mnt/c/.../npm).
  marker="# >>> cogit build env >>>"
  if ! grep -qF "$marker" "$HOME/.profile" 2>/dev/null; then
    say "PATH and build env in ~/.profile"
    cat >> "$HOME/.profile" <<'EOF'
# >>> cogit build env >>>
export PATH="$HOME/.cargo/bin:$HOME/.local/node/bin:$PATH"
# .cargo/config.toml names sccache as the rustc wrapper; empty = off (use rust-cache or install it).
export RUSTC_WRAPPER="${RUSTC_WRAPPER-}"
# <<< cogit build env <<<
EOF
  fi
}

case "$mode" in
  --system) system_part ;;
  --user)   user_part ;;
  all)      system_part; user_part ;;
  *) echo "usage: $0 [--system|--user]" >&2; exit 2 ;;
esac
say "done"
