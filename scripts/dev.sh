#!/usr/bin/env bash
# Runs Studio in development with hot reload, using a toolchain that lives entirely inside
# this repository (.toolchain/, gitignored). Nothing is installed globally.
#
#   ./scripts/dev.sh            bootstrap if needed, then run the app
#   ./scripts/dev.sh --setup    bootstrap only
#   ./scripts/dev.sh --browser  run only the UI in a browser with sample data
#
# First run downloads Node and Rust (roughly 1 GB including the first build) into
# .toolchain/ and FFmpeg into node_modules/. Later runs start in seconds. While it runs,
# UI changes appear instantly and Rust changes rebuild and relaunch the app automatically.
#
# Requirement outside the repo: Apple's command line tools (compilers and linker), which
# Rust needs on macOS. Install once with `xcode-select --install`.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TC="$ROOT/.toolchain"
NODE_MAJOR=22

export RUSTUP_HOME="$TC/rustup"
export CARGO_HOME="$TC/cargo"
export PATH="$TC/node/bin:$CARGO_HOME/bin:$PATH"
# Keep npm's cache inside the repo too.
export npm_config_cache="$TC/npm-cache"

log() { printf '\033[1;33m[studio]\033[0m %s\n' "$*"; }
fail() { printf '\033[1;31m[studio]\033[0m %s\n' "$*" >&2; exit 1; }

platform() {
  local os arch
  case "$(uname -s)" in
    Darwin) os=darwin ;;
    Linux) os=linux ;;
    *) fail "Unsupported OS: $(uname -s)" ;;
  esac
  case "$(uname -m)" in
    arm64 | aarch64) arch=arm64 ;;
    x86_64) arch=x64 ;;
    *) fail "Unsupported CPU: $(uname -m)" ;;
  esac
  echo "$os-$arch"
}

check_system() {
  if [[ "$(uname -s)" == Darwin ]] && ! xcode-select -p >/dev/null 2>&1; then
    fail "Apple command line tools are missing. Run: xcode-select --install, then re-run this script."
  fi
  command -v curl >/dev/null || fail "curl is required."
  command -v shasum >/dev/null || command -v sha256sum >/dev/null || fail "shasum or sha256sum is required."
}

sha256() {
  if command -v shasum >/dev/null; then shasum -a 256 "$1" | cut -d' ' -f1; else sha256sum "$1" | cut -d' ' -f1; fi
}

install_node() {
  if [[ -x "$TC/node/bin/node" ]]; then return; fi
  local plat base sums file expected tmp
  plat="$(platform)"
  base="https://nodejs.org/dist/latest-v${NODE_MAJOR}.x"
  log "Installing Node ${NODE_MAJOR} into .toolchain/node"
  tmp="$(mktemp -d)"
  curl -fsSL "$base/SHASUMS256.txt" -o "$tmp/SHASUMS256.txt"
  file="$(grep -oE "node-v[0-9.]+-${plat}\.tar\.gz" "$tmp/SHASUMS256.txt" | head -n1)"
  [[ -n "$file" ]] || fail "No Node ${NODE_MAJOR} build for ${plat}."
  expected="$(grep " ${file}\$" "$tmp/SHASUMS256.txt" | cut -d' ' -f1)"
  curl -fSL --progress-bar "$base/$file" -o "$tmp/$file"
  [[ "$(sha256 "$tmp/$file")" == "$expected" ]] || fail "Node download failed its checksum."
  mkdir -p "$TC/node"
  tar -xzf "$tmp/$file" -C "$TC/node" --strip-components=1
  rm -rf "$tmp"
}

install_rust() {
  if [[ -x "$CARGO_HOME/bin/cargo" ]]; then return; fi
  log "Installing Rust into .toolchain (rustup, pinned by rust-toolchain.toml)"
  mkdir -p "$TC"
  curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs -o "$TC/rustup-init.sh"
  sh "$TC/rustup-init.sh" -y --no-modify-path --profile minimal --default-toolchain none
  rm -f "$TC/rustup-init.sh"
  # Installs the exact channel from rust-toolchain.toml.
  (cd "$ROOT" && rustup show active-toolchain >/dev/null 2>&1 || rustup toolchain install)
}

install_packages() {
  local stamp="$TC/.npm-installed"
  if [[ -d "$ROOT/node_modules" && -f "$stamp" && "$stamp" -nt "$ROOT/package-lock.json" ]]; then return; fi
  log "Installing npm packages into node_modules (includes FFmpeg)"
  (cd "$ROOT" && npm ci --no-audit --no-fund)
  touch "$stamp"
}

use_bundled_ffmpeg() {
  # Point the app at the FFmpeg binaries installed in node_modules unless overridden.
  if [[ -z "${STUDIO_FFMPEG:-}" ]]; then
    STUDIO_FFMPEG="$(cd "$ROOT" && node -p "require('ffmpeg-static')")"
    export STUDIO_FFMPEG
  fi
  if [[ -z "${STUDIO_FFPROBE:-}" ]]; then
    STUDIO_FFPROBE="$(cd "$ROOT" && node -p "require('@ffprobe-installer/ffprobe').path")"
    export STUDIO_FFPROBE
  fi
  [[ -x "$STUDIO_FFMPEG" ]] || fail "FFmpeg was not installed (expected $STUDIO_FFMPEG)."
  [[ -x "$STUDIO_FFPROBE" ]] || fail "ffprobe was not installed (expected $STUDIO_FFPROBE)."
}

main() {
  local mode="${1:-run}"
  check_system
  install_node
  install_packages
  case "$mode" in
    --browser)
      log "UI only, in the browser with sample data: http://localhost:1420"
      cd "$ROOT" && exec npm run dev
      ;;
  esac
  install_rust
  use_bundled_ffmpeg
  if [[ "$mode" == "--setup" ]]; then
    log "Toolchain ready in .toolchain/. Run ./scripts/dev.sh to start Studio."
    return
  fi
  log "Starting Studio with hot reload (Ctrl+C to stop)"
  cd "$ROOT" && exec npm run app
}

main "$@"
