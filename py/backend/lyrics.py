from __future__ import annotations

import sys
import re
from dataclasses import dataclass
from pathlib import Path

VENDOR = Path(__file__).resolve().parents[1] / "vendor"
if str(VENDOR) not in sys.path:
    sys.path.insert(0, str(VENDOR))

from pyncm.utils.lrcparser import LrcParser  # noqa: E402


@dataclass(frozen=True)
class LyricSegment:
    start: float
    end: float
    text: str


@dataclass(frozen=True)
class LyricLine:
    time: float
    text: str
    translation: str = ""
    romaji: str = ""
    segments: tuple[LyricSegment, ...] = ()


def _parse_map(text: str) -> dict[float, str]:
    if not text.strip():
        return {}
    parsed = LrcParser(text)
    result: dict[float, str] = {}
    for stamp, entries in parsed.lyrics_sorted.items():
        joined = "".join(value for _, value in entries).strip()
        if joined:
            result[round(float(stamp), 3)] = joined
    return result


def parse_lyrics(text: str, translation: str = "") -> list[LyricLine]:
    return parse_rich_lyrics(text, translation)


def parse_rich_lyrics(
    text: str, translation: str = "", romanization: str = "", karaoke: str = ""
) -> list[LyricLine]:
    source = _parse_map(text)
    trans = _parse_map(translation)
    roman = _parse_map(romanization)
    timing = _parse_enhanced_map(karaoke)
    lines = [
        LyricLine(
            stamp,
            value,
            trans.get(stamp, ""),
            roman.get(stamp, ""),
            tuple(timing.get(stamp, ())),
        )
        for stamp, value in sorted(source.items())
    ]
    return lines


def _parse_enhanced_map(text: str) -> dict[float, tuple[LyricSegment, ...]]:
    line_pattern = re.compile(r"^\[(\d+):(\d+(?:\.\d+)?)\](.*)$")
    word_pattern = re.compile(r"<(\d+),(\d+),-?\d+>([^<]*)")
    result = {}
    for raw in text.splitlines():
        match = line_pattern.match(raw.strip())
        if not match:
            continue
        minutes, seconds, body = match.groups()
        line_time = int(minutes) * 60 + float(seconds)
        segments = []
        for start_ms, duration_ms, word in word_pattern.findall(body):
            start = line_time + int(start_ms) / 1000.0
            end = start + max(int(duration_ms), 1) / 1000.0
            if word:
                segments.append(LyricSegment(start, end, word))
        if segments:
            result[round(line_time, 3)] = tuple(segments)
    return result


def locate(lines: list[LyricLine], t: float) -> tuple[int, float]:
    if not lines:
        return 0, 0.0
    if t <= lines[0].time:
        return 0, 0.0
    lo, hi = 0, len(lines) - 1
    while lo <= hi:
        mid = (lo + hi) // 2
        if lines[mid].time <= t:
            lo = mid + 1
        else:
            hi = mid - 1
    idx = max(0, hi)
    start = lines[idx].time
    end = lines[idx + 1].time if idx + 1 < len(lines) else start + 5.0
    progress = (t - start) / max(end - start, 0.1)
    return idx, min(max(progress, 0.0), 1.0)
