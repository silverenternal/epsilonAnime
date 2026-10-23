#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

binary="$root/target/release/epsilonAnime"
if [[ ! -x "$binary" ]]; then
  binary="$root/epsilonAnime"
fi
if [[ ! -x "$binary" ]]; then
  cargo build --release
  binary="$root/target/release/epsilonAnime"
fi
if [[ ! -x py/.venv/bin/python ]]; then
  (cd py && uv sync)
fi
exec "$binary" "$@"
