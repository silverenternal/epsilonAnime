from __future__ import annotations

import argparse
import json
import os
import sys
import threading
import time
import traceback
from pathlib import Path

from .cache import scrub_persisted_urls
from .cover import CoverInfo, materialize, register_cleanup
from .control import ControlBus
from .emitter import JsonEmitter
from .lyrics import parse_rich_lyrics
from .ncm import Track, fetch_comments, fetch_track, refresh_track_audio
from . import offline
from . import player


def watch_parent_process() -> None:
    """Terminate audio if the TUI parent disappears without normal cleanup."""
    original_parent = os.getppid()
    while True:
        if os.getppid() != original_parent:
            os._exit(0)
        time.sleep(0.5)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="happybirthday NetEase backend")
    parser.add_argument("url", nargs="?", help="NetEase song URL")
    parser.add_argument("--level", default="standard", choices=["standard", "exhigh", "lossless", "hires"])
    parser.add_argument("--cookie", default=None, help="MUSIC_U cookie for VIP tracks")
    parser.add_argument("--analyze-only", action="store_true", help="decode/analyze without playback")
    parser.add_argument("--max-seconds", type=float, default=None)
    parser.add_argument("--sample-rate", type=int, default=44100)
    return parser


def run(
    args: argparse.Namespace,
    emitter: JsonEmitter,
    control: ControlBus,
    preload: bool = False,
) -> None:
    scrub_persisted_urls(Path(__file__).resolve().parents[2] / "target" / "netease-cache")
    track, cover, comments = load_cached(args)
    if track is None:
        track = fetch_track(args.url, level=args.level, cookie=args.cookie)
        cover = materialize(track.cover_url)
        comments = _comments(track.song_id)
    else:
        if not track.audio_url:
            track = refresh_track_audio(track, level=args.level, cookie=args.cookie)
        if not comments:
            comments = _comments(track.song_id)
    print(
        f"track: {track.title} / {track.artist} ({track.song_id})",
        file=sys.stderr,
        flush=True,
    )
    prepared_stream = None
    if preload:
        prepared_stream = player.preload_stream(track, args.sample_rate)
        if control.wait_for("start", timeout=90.0) is None:
            source, stream, _ = prepared_stream
            stream.close()
            source.close()
            raise TimeoutError("preloaded track was not scheduled within 90 seconds")
        print("audio: preloaded stream started", file=sys.stderr, flush=True)
    offline.record_recent(
        {
            "id": track.song_id,
            "title": track.title,
            "artist": track.artist,
            "duration_ms": track.duration_ms,
        }
    )
    emitter.emit(
        {
            "type": "meta",
            "title": track.title,
            "artist": track.artist,
            "duration_ms": track.duration_ms,
            "cover_path": cover.path,
            "cover_cols": cover.cols,
            "cover_rows": cover.rows,
            "particle_path": cover.particle_path,
            "particle_count": cover.particle_count,
        }
    )
    emitter.emit({"type": "comments", "comments": comments})
    emitter.emit(
        {
            "type": "cover_ready",
            "path": cover.path,
            "cols": cover.cols,
            "rows": cover.rows,
            "particle_path": cover.particle_path,
            "particle_count": cover.particle_count,
        }
    )
    lines = parse_rich_lyrics(
        track.lyric_text,
        track.translation_text,
        track.romaji_text,
        track.karaoke_text,
    )
    print(f"lyrics: {len(lines)} timed lines", file=sys.stderr, flush=True)
    emitter.emit(
        {
            "type": "lyrics_ready",
            "lines": [
                {
                    "time": line.time,
                    "text": line.text,
                    "translation": line.translation,
                    "romaji": line.romaji,
                    "segments": [
                        {
                            "start": segment.start,
                            "end": segment.end,
                            "text": segment.text,
                        }
                        for segment in line.segments
                    ],
                }
                for line in lines
            ],
        }
    )
    player.play(
        track,
        lines,
        emitter.emit,
        analyze_only=args.analyze_only,
        max_seconds=args.max_seconds,
        sample_rate=args.sample_rate,
        control=control,
        level=args.level,
        cookie=args.cookie,
        prepared_stream=prepared_stream,
    )
    emitter.emit({"type": "state", "state": "ended"})


def load_cached(
    args: argparse.Namespace,
) -> tuple[Track | None, CoverInfo | None, list[dict]]:
    path = Path(args.cache_path) if args.cache_path else None
    if path is None or not path.exists():
        return None, None, []
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
        if time.time() - float(payload["cached_at"]) > 600:
            return None, None, []
        cover = CoverInfo(**payload["cover"])
        if not os.path.exists(cover.path) or not os.path.exists(cover.particle_path):
            return None, None, []
        register_cleanup(cover.path)
        register_cleanup(cover.particle_path)
        return Track(**payload["track"]), cover, list(payload.get("comments") or [])
    except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError):
        return None, None, []


def _comments(song_id: int) -> list[dict]:
    try:
        return fetch_comments(song_id)
    except Exception as exc:
        print(f"comments unavailable for {song_id}: {exc}", file=sys.stderr)
        return []


def main() -> int:
    args = build_parser().parse_args()
    threading.Thread(
        target=watch_parent_process,
        name="parent-watchdog",
        daemon=True,
    ).start()
    args.cache_path = None
    args.preload = False
    if not args.url:
        line = sys.stdin.readline().strip()
        try:
            payload = json.loads(line)
            args.url = payload["url"]
            args.cache_path = payload.get("cache_path")
            args.preload = bool(payload.get("preload", False))
        except (json.JSONDecodeError, KeyError) as exc:
            print(f"invalid prewarm payload: {exc}", file=sys.stderr)
            return 2
    emitter = JsonEmitter()
    control = ControlBus()
    control.start()
    try:
        run(args, emitter, control, preload=args.preload)
        return 0
    except KeyboardInterrupt:
        return 130
    except Exception as exc:
        emitter.emit(
            {
                "type": "error",
                "message": str(exc),
                "id": type(exc).__name__,
                "scope": "backend",
                "level": "error",
                "retryable": True,
            }
        )
        traceback.print_exc(file=sys.stderr)
        return 1
    finally:
        emitter.close()


if __name__ == "__main__":
    raise SystemExit(main())
