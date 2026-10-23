from __future__ import annotations

import argparse
import json
import os
import sys
import time
from dataclasses import asdict
from pathlib import Path

from .cover import materialize
from .cache import prune_cache
from .ncm import fetch_comments, fetch_track


def main() -> int:
    parser = argparse.ArgumentParser(description="Prefetch a NetEase track")
    parser.add_argument("url")
    parser.add_argument("cache_path")
    args = parser.parse_args()
    try:
        track = fetch_track(args.url)
        cover = materialize(track.cover_url, cleanup=False)
        comments = fetch_comments(track.song_id)
        track_payload = asdict(track)
        track_payload["audio_url"] = ""
        track_payload["audio_type"] = ""
        payload = {
            "cached_at": time.time(),
            "url": args.url,
            "track": track_payload,
            "cover": asdict(cover),
            "comments": comments,
        }
        path = Path(args.cache_path)
        path.parent.mkdir(parents=True, exist_ok=True)
        tmp = path.with_suffix(f".{os.getpid()}.tmp")
        tmp.write_text(json.dumps(payload, ensure_ascii=False), encoding="utf-8")
        os.replace(tmp, path)
        prune_cache(path.parent)
        print(f"prefetched {track.title} -> {path}", file=sys.stderr)
        return 0
    except Exception as exc:
        print(f"prefetch failed: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
