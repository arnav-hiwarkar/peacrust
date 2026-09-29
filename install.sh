#!/bin/sh
# Build and install peacrust, then offer to install cava.
set -eu
cd "$(dirname "$0")"
if ! command -v cargo >/dev/null 2>&1; then
  echo "peacrust needs Rust. Install it from https://rustup.rs and run ./install.sh again." >&2
  exit 1
fi
cargo install --path . --locked
bin="${CARGO_HOME:-$HOME/.cargo}/bin/peacrust"
if ! command -v cava >/dev/null 2>&1; then
  "$bin" --install-cava || echo "cava was not installed. peacrust still works; the visualizer panel will show how to install it."
fi
echo "Installed. Run 'peacrust', or try a preset: peacrust presets/neon"
