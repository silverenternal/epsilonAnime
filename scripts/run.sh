#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ ! -x py/.venv/bin/python ]]; then
  (cd py && uv sync)
fi
cargo build --release
exec target/release/epsilonAnime "$@"
