from __future__ import annotations

import os
import re
import stat
import sys
import time
from dataclasses import dataclass, replace
from pathlib import Path

VENDOR = Path(__file__).resolve().parents[1] / "vendor"
if str(VENDOR) not in sys.path:
    sys.path.insert(0, str(VENDOR))

from pyncm import GetCurrentSession, apis  # noqa: E402
from pyncm.apis.login import LoginViaCookie  # noqa: E402


@dataclass
class Track:
    song_id: int
    title: str
    artist: str
    duration_ms: int
    cover_url: str
    lyric_text: str
    translation_text: str
    audio_url: str
    audio_type: str
    gain: float
    peak: float
    romaji_text: str = ""
    karaoke_text: str = ""


def parse_song_id(url: str) -> int:
    patterns = [
        r"[?&]id=(\d+)",
        r"/song/(\d+)",
        r"^(\d+)$",
    ]
    for pattern in patterns:
        match = re.search(pattern, url.strip())
        if match:
            return int(match.group(1))
    raise ValueError(f"cannot extract NetEase song id from {url!r}")


def _session(cookie: str | None = None):
    session = GetCurrentSession()
    session.headers["X-Real-IP"] = os.getenv("NETEASE_REAL_IP", "118.88.88.88")
    token = cookie or os.getenv("NETEASE_COOKIE", "")
    session_file = os.getenv("NETEASE_SESSION_FILE") or str(
        Path(__file__).resolve().parents[2] / "target" / "netease-session.json"
    )
    if not token and os.path.exists(session_file):
        try:
            if stat.S_IMODE(os.stat(session_file).st_mode) & 0o077:
                os.chmod(session_file, 0o600)
            import json

            token = json.loads(Path(session_file).read_text()).get("MUSIC_U", "")
        except (OSError, ValueError):
            token = ""
    if token:
        session.cookies.update({"MUSIC_U": token})
        LoginViaCookie(token)
    return session


def fetch_track(url: str, level: str = "standard", cookie: str | None = None) -> Track:
    song_id = parse_song_id(url)
    _session(cookie)
    detail = apis.track.GetTrackDetail([song_id])
    songs = detail.get("songs") or []
    if not songs:
        raise RuntimeError(f"song detail unavailable: {detail.get('code', detail)}")
    song = songs[0]
    album = song.get("al") or {}
    artists = " / ".join(a.get("name", "") for a in song.get("ar") or [])

    item = _audio_item(song_id, level)
    lrc, trans, romaji, karaoke = _lyrics(song_id)

    return Track(
        song_id=song_id,
        title=song.get("name") or str(song_id),
        artist=artists or "Unknown artist",
        duration_ms=int(song.get("dt") or item.get("time") or 0),
        cover_url=album.get("picUrl") or "",
        lyric_text=lrc.get("lyric") or "",
        translation_text=trans.get("lyric") or "",
        romaji_text=romaji.get("lyric") or "",
        karaoke_text=karaoke.get("lyric") or "",
        audio_url=item["url"],
        audio_type=_audio_type(item, item["url"]),
        gain=float(item.get("gain") or 0.0),
        peak=float(item.get("peak") or 1.0),
    )


def _audio_item(song_id: int, level: str = "standard") -> dict:
    audio = apis.track.GetTrackAudioV1([song_id], level=level, encodeType="mp3")
    items = audio.get("data") or []
    if items and items[0].get("url"):
        try:
            _audio_type(items[0], items[0]["url"])
            return items[0]
        except RuntimeError:
            pass
    legacy = apis.track.GetTrackAudio([song_id], bitrate=128000, encodeType="mp3")
    fallback = legacy.get("data") or []
    if fallback and fallback[0].get("url"):
        _audio_type(fallback[0], fallback[0]["url"])
        return fallback[0]
    reason = items[0].get("freeTrialInfo") if items else legacy
    raise RuntimeError(f"audio URL unavailable (VIP/login required?): {reason}")


def _lyrics(song_id: int) -> tuple[dict, dict, dict, dict]:
    try:
        lyric = apis.track.GetTrackLyricsNew(str(song_id))
        lrc = lyric.get("lrc") or {}
        trans = lyric.get("tlyric") or {}
        romaji = lyric.get("romalrc") or {}
        karaoke = lyric.get("klyric") or lyric.get("yrc") or {}
        if not lrc.get("lyric"):
            legacy = apis.track.GetTrackLyrics(str(song_id))
            lrc = legacy.get("lrc") or {}
            trans = legacy.get("tlyric") or {}
            romaji = legacy.get("romalrc") or {}
        return lrc, trans, romaji, karaoke
    except Exception as exc:
        print(f"lyrics unavailable for {song_id}: {exc}", file=sys.stderr)
        return {}, {}, {}, {}


def _audio_type(item: dict, url: str) -> str:
    kind = str(item.get("type") or "").lower()
    if kind in {"mp3", "flac", "wav", "ogg", "vorbis"}:
        return "ogg" if kind == "vorbis" else kind
    path = url.split("?", 1)[0].lower()
    for extension in ("mp3", "flac", "wav", "ogg"):
        if path.endswith(f".{extension}"):
            return extension
    raise RuntimeError(f"unsupported NetEase audio format: {kind or url[:80]}")


def refresh_track_audio(
    track: Track, level: str = "standard", cookie: str | None = None
) -> Track:
    """Refresh the expiring media URL while preserving cached metadata and lyrics."""
    _session(cookie)
    item = _audio_item(track.song_id, level)
    return replace(
        track,
        audio_url=item["url"],
        audio_type=_audio_type(item, item["url"]),
        gain=float(item.get("gain") or 0.0),
        peak=float(item.get("peak") or 1.0),
    )


def fetch_comments(song_id: int, limit: int = 40) -> list[dict]:
    response = apis.track.GetTrackComments(song_id, offset=0, limit=min(limit * 2, 100))
    items = list(response.get("hotComments") or []) + list(response.get("comments") or [])
    seen = set()
    comments = []
    for item in items:
        comment_id = item.get("commentId")
        if comment_id in seen:
            continue
        seen.add(comment_id)
        text = " ".join(str(item.get("content") or "").split())
        if not text:
            continue
        comments.append(
            {
                "text": text[:180],
                "user": (item.get("user") or {}).get("nickname") or "网易云用户",
                "likes": int(item.get("likedCount") or 0),
            }
        )
        if len(comments) >= limit:
            break
    return comments


def qr_login(timeout: float = 120.0) -> bool:
    """Optional interactive QR login hook.

    The visualizer does not need it for free tracks. VIP/lossless tracks may
    require a cookie, which is accepted by fetch_track(cookie=...).
    """
    key = apis.login.LoginQrcodeUnikey()["unikey"]
    print(f"Scan: https://music.163.com/login?codekey={key}", file=sys.stderr)
    deadline = time.time() + timeout
    while time.time() < deadline:
        status = apis.login.LoginQrcodeCheck(key)
        if status.get("code") == 803:
            return True
        time.sleep(2.0)
    return False
