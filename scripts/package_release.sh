#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
tag="${1:?usage: scripts/package_release.sh vX.Y.Z}"
cd "$root"

if [[ ! "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]]; then
  echo "invalid release tag: $tag" >&2
  exit 2
fi

case "${RUNNER_OS:-$(uname -s)}" in
  Linux) platform=linux ;;
  macOS|Darwin) platform=macos ;;
  Windows|MINGW*|MSYS*) platform=windows ;;
  *)
    echo "unsupported release platform" >&2
    exit 2
    ;;
esac

case "$(uname -m)" in
  arm64|aarch64) arch=aarch64 ;;
  x86_64|amd64) arch=x86_64 ;;
  *)
    echo "unsupported release architecture: $(uname -m)" >&2
    exit 2
    ;;
esac

name="epsilonAnime-$tag-$platform-$arch"
stage="$root/dist/$name"
case "$stage" in
  "$root"/dist/*) ;;
  *)
    echo "unsafe package path: $stage" >&2
    exit 2
    ;;
esac

rm -rf -- "$stage"
mkdir -p "$stage"

if [[ "$platform" == windows ]]; then
  cp target/release/epsilonAnime.exe "$stage/"
else
  cp target/release/epsilonAnime "$stage/"
fi
cp README.md LICENSE CONTRIBUTING.md SECURITY.md config.example.toml "$stage/"
cp -R docs scripts specs "$stage/"
mkdir "$stage/py"
find "$root/py" -mindepth 1 -maxdepth 1 ! -name .venv \
  -exec cp -R {} "$stage/py/" \;
find "$stage/py" -type d -name __pycache__ -prune -exec rm -rf -- {} +

if [[ "$platform" == windows ]]; then
  archive="$stage.zip"
  rm -f -- "$archive"
  relative_stage="dist/$name"
  pwsh -NoProfile -Command \
    "Compress-Archive -LiteralPath '$relative_stage' -DestinationPath '$relative_stage.zip' -CompressionLevel Optimal"
else
  archive="$stage.tar.gz"
  tar -C "$root/dist" -czf "$archive" "$name"
fi

echo "$archive"
