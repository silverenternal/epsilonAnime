from __future__ import annotations

import json
import os
import tempfile
from pathlib import Path

MAX_FILES = 128
MAX_BYTES = 512 * 1024 * 1024


def prune_cache(
    root: Path,
    max_files: int = MAX_FILES,
    max_bytes: int = MAX_BYTES,
) -> int:
    if not root.exists():
        return 0
    entries = sorted(root.glob("*.json"), key=lambda path: path.stat().st_mtime)
    total = sum(path.stat().st_size for path in entries)
    removed = 0
    while entries and (len(entries) > max_files or total > max_bytes):
        path = entries.pop(0)
        payload = _read(path)
        total -= path.stat().st_size
        try:
            path.unlink()
            removed += 1
        except OSError:
            continue
        _remove_cover(payload, root)
    return removed


def scrub_persisted_urls(root: Path) -> int:
    changed = 0
    for path in root.glob("*.json"):
        payload = _read(path)
        track = payload.get("track")
        if not isinstance(track, dict):
            continue
        if track.get("audio_url") or track.get("audio_type"):
            track["audio_url"] = ""
            track["audio_type"] = ""
            try:
                path.write_text(json.dumps(payload, ensure_ascii=False), encoding="utf-8")
                changed += 1
            except OSError:
                pass
    return changed


def clear_cache(root: Path) -> int:
    removed = 0
    for path in list(root.glob("*.json")):
        payload = _read(path)
        _remove_cover(payload, root)
        try:
            path.unlink()
            removed += 1
        except OSError:
            pass
    return removed


def _read(path: Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError, TypeError):
        return {}


def _remove_cover(payload: dict, cache_root: Path) -> None:
    cover = payload.get("cover") or {}
    kept = set()
    for path in cache_root.glob("*.json"):
        other = _read(path)
        other_cover = other.get("cover") or {}
        kept.update(
            value
            for value in (other_cover.get("path"), other_cover.get("particle_path"))
            if value
        )
    temp_root = Path(tempfile.gettempdir()).resolve()
    for value in (cover.get("path"), cover.get("particle_path")):
        if not value or value in kept:
            continue
        path = Path(value)
        try:
            resolved = path.resolve()
            if resolved.is_relative_to(cache_root.resolve()):
                pass
            else:
                resolved.relative_to(temp_root)
        except (OSError, ValueError):
            continue
        if path.name.startswith(("happybirthday-cover-", "happybirthday-particles-")):
            try:
                os.remove(path)
            except OSError:
                pass
