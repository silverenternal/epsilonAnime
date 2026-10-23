from __future__ import annotations

import argparse
import json
import os
import sys
import time
from pathlib import Path

import qrcode

from .emitter import JsonEmitter
from . import library_api
from . import offline
from .cache import clear_cache
from .ncm import _session

from pyncm import GetCurrentSession, apis  # noqa: E402


SESSION_FILE = Path(__file__).resolve().parents[2] / "target" / "netease-session.json"


def emit(emitter: JsonEmitter, event: dict) -> None:
    emitter.emit(event)


def save_session() -> None:
    SESSION_FILE.parent.mkdir(parents=True, exist_ok=True)
    cookies = GetCurrentSession().cookies.get_dict()
    SESSION_FILE.write_text(json.dumps({"MUSIC_U": cookies.get("MUSIC_U", "")}))
    os.chmod(SESSION_FILE, 0o600)


def clear_session() -> None:
    try:
        SESSION_FILE.unlink()
    except FileNotFoundError:
        pass
    GetCurrentSession().cookies.clear()


def user_and_playlists(emitter: JsonEmitter) -> dict:
    status = apis.login.GetCurrentLoginStatus()
    content = status.get("content") or status
    profile = content.get("profile") or status.get("profile") or {}
    account = content.get("account") or status.get("account") or {}
    uid = profile.get("userId") or account.get("id") or 0
    nickname = profile.get("nickname") or GetCurrentSession().nickname or "网易云用户"
    signature = profile.get("signature") or ""
    emit(
        emitter,
        {
            "type": "user",
            "uid": int(uid or 0),
            "nickname": nickname,
            "signature": signature,
        },
    )
    playlist_rows = user_playlist_rows(int(uid or 0))
    if not uid:
        emit(
            emitter,
            {
                "type": "error",
                "message": "网易云登录态不可用，请按 g 重新扫码",
            },
        )
    items = []
    for item in playlist_rows:
        items.append(
            {
                "id": int(item.get("id") or 0),
                "name": item.get("name") or "未命名歌单",
                "count": int(item.get("trackCount") or 0),
                "cover": item.get("coverImgUrl") or "",
                "kind": "playlist",
            }
        )
    emit(emitter, {"type": "playlists", "items": items})
    offline.store("user:playlists", {"type": "playlists", "items": items})
    return {"uid": uid, "nickname": nickname}


def qr_lines(url: str) -> list[str]:
    code = qrcode.QRCode(border=2, box_size=1)
    code.add_data(url)
    code.make(fit=True)
    matrix = code.get_matrix()
    lines = []
    dots = (
        (0, 0, 0x01),
        (0, 1, 0x02),
        (0, 2, 0x04),
        (1, 0, 0x08),
        (1, 1, 0x10),
        (1, 2, 0x20),
        (0, 3, 0x40),
        (1, 3, 0x80),
    )
    for y in range(0, len(matrix), 4):
        line = []
        for x in range(0, len(matrix[0]), 2):
            bits = 0
            for dx, dy, bit in dots:
                if y + dy < len(matrix) and x + dx < len(matrix[y + dy]):
                    if matrix[y + dy][x + dx]:
                        bits |= bit
            line.append(chr(0x2800 + bits))
        lines.append("".join(line))
    return lines


def qr_login(emitter: JsonEmitter) -> None:
    token = apis.login.LoginQrcodeUnikey()
    unikey = token.get("unikey")
    if not unikey:
        emit(emitter, {"type": "error", "message": "无法创建二维码登录会话"})
        return
    url = apis.login.GetLoginQRCodeUrl(unikey)
    lines = qr_lines(url)
    print(
        f"qr: generated {len(lines)}x{len(lines[0]) if lines else 0} braille rows",
        file=sys.stderr,
        flush=True,
    )
    emit(emitter, {"type": "qr", "url": url, "lines": lines})
    deadline = time.time() + 180
    while time.time() < deadline:
        status = apis.login.LoginQrcodeCheck(unikey)
        code = status.get("code")
        if code == 803:
            save_session()
            user_and_playlists(emitter)
            return
        if code == 800:
            emit(emitter, {"type": "error", "message": "二维码已过期，请重新登录"})
            return
        time.sleep(2.0)
    emit(emitter, {"type": "error", "message": "二维码登录超时"})


def playlist_songs(emitter: JsonEmitter, value: str) -> None:
    kind, collection_id, offset = parse_collection_value(value)
    _cached_emit(
        emitter,
        f"collection:{kind}:{collection_id}:{offset}",
        lambda: library_api.collection(kind, collection_id, offset),
    )


def parse_collection_value(value: str) -> tuple[str, int, int]:
    parts = value.rsplit(":", 2)
    if len(parts) == 3:
        return parts[0], int(parts[1]), int(parts[2])
    if len(parts) == 2:
        return "playlist", int(parts[0]), int(parts[1])
    return "playlist", int(parts[0]), 0


def browse(emitter: JsonEmitter, value: str) -> None:
    parts = value.split(":")
    kind = parts[0] if parts else "charts"
    offset = int(parts[1]) if len(parts) > 1 else 0
    if kind == "daily":
        _cached_emit(emitter, f"browse:{kind}:{offset}", lambda: library_api.daily(offset))
    elif kind == "radio":
        _cached_emit(
            emitter,
            f"browse:{kind}:{offset}",
            lambda: library_api.collection("radio", 0, offset),
        )
    elif kind == "liked":
        profile = require_login()
        playlists = user_playlist_rows(profile["uid"])
        _cached_emit(
            emitter,
            f"browse:{kind}:{offset}",
            lambda: library_api.liked(playlists, offset),
        )
    elif kind == "search":
        return
    elif kind == "recent":
        emit(
            emitter,
            {
                "type": "songs",
                "kind": "recent",
                "playlist_id": 0,
                "offset": 0,
                "total": len(offline.recent()),
                "items": offline.recent(),
                "title": "最近播放",
            },
        )
        return
    else:
        _cached_emit(
            emitter,
            f"browse:{kind}:{offset}",
            lambda: library_api.user_collections(kind),
        )


def _cached_emit(emitter: JsonEmitter, key: str, build) -> None:
    try:
        event = build()
    except Exception:
        cached = offline.load(key)
        if cached is None:
            raise
        cached["cached"] = True
        emit(emitter, cached)
        return
    offline.store(key, event)
    event["cached"] = False
    emit(emitter, event)


def user_profile() -> dict:
    status = apis.login.GetCurrentLoginStatus()
    content = status.get("content") or status
    profile = content.get("profile") or status.get("profile") or {}
    account = content.get("account") or status.get("account") or {}
    uid = int(profile.get("userId") or account.get("id") or 0)
    return {
        "uid": uid,
        "nickname": profile.get("nickname") or GetCurrentSession().nickname or "网易云用户",
        "signature": profile.get("signature") or "",
    }


def require_login() -> dict:
    profile = user_profile()
    if not profile["uid"]:
        clear_session()
        raise RuntimeError("网易云登录已失效，请按 g 重新扫码（auth_expired）")
    return profile


def user_playlist_rows(uid: int) -> list[dict]:
    if not uid:
        return []
    page_size = 500
    offset = 0
    seen = set()
    rows = []
    while offset < 10_000:
        response = apis.user.GetUserPlaylists(uid, offset=offset, limit=page_size)
        page = list(response.get("playlist") or [])
        for item in page:
            item_id = int(item.get("id") or 0)
            if item_id and item_id not in seen:
                seen.add(item_id)
                rows.append(item)
        if len(page) < page_size:
            break
        offset += len(page)
    return rows


def logout(emitter: JsonEmitter) -> None:
    clear_session()
    clear_cache(Path(__file__).resolve().parents[2] / "target" / "netease-cache")
    offline.clear()
    emit(emitter, {"type": "logged_out"})


def detail(emitter: JsonEmitter, value: str) -> None:
    kind, collection_id = value.rsplit(":", 1)
    _cached_emit(
        emitter,
        f"detail:{kind}:{collection_id}",
        lambda: library_api.detail(kind, int(collection_id)),
    )


def like_current(emitter: JsonEmitter, value: str) -> None:
    profile = require_login()
    emit(emitter, library_api.like(int(value), int(profile["uid"]), True))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "command",
        choices=[
            "qr",
            "playlists",
            "songs",
            "browse",
            "search",
            "detail",
            "like",
            "logout",
        ],
    )
    parser.add_argument("value", nargs="?")
    args = parser.parse_args()
    emitter = JsonEmitter()
    try:
        _session()
        if args.command == "qr":
            qr_login(emitter)
        elif args.command == "playlists":
            try:
                user_and_playlists(emitter)
            except Exception:
                cached = offline.load("user:playlists")
                if cached is None:
                    raise
                cached["cached"] = True
                emit(emitter, cached)
        elif args.command == "browse":
            browse(emitter, args.value or "charts")
        elif args.command == "search":
            raw = args.value or ""
            prefix, separator, query = raw.partition(":")
            if separator and prefix in {"song", "album", "artist", "playlist", "mv"}:
                _cached_emit(
                    emitter,
                    f"search:{prefix}:{query}",
                    lambda: library_api.search(query, kind=prefix),
                )
            else:
                _cached_emit(
                    emitter,
                    f"search:song:{raw}",
                    lambda: library_api.search(raw),
                )
        elif args.command == "logout":
            logout(emitter)
        elif args.command == "detail":
            detail(emitter, args.value or "")
        elif args.command == "like":
            like_current(emitter, args.value or "0")
        else:
            playlist_songs(emitter, args.value or "playlist:0")
        return 0
    except Exception as exc:
        emit(
            emitter,
            {
                "type": "error",
                "message": str(exc),
                "id": type(exc).__name__,
                "code": "auth_expired" if "失效" in str(exc) else type(exc).__name__,
                "scope": "auth",
                "level": "error",
                "retryable": "失效" not in str(exc),
            },
        )
        print(exc, file=sys.stderr)
        return 1
    finally:
        emitter.close()


if __name__ == "__main__":
    raise SystemExit(main())
