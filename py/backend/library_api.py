from __future__ import annotations

import sys
from pathlib import Path

VENDOR = Path(__file__).resolve().parents[1] / "vendor"
if str(VENDOR) not in sys.path:
    sys.path.insert(0, str(VENDOR))

from pyncm import apis
from pyncm.apis import WeapiCryptoRequest
from pyncm.apis.miniprograms import radio

PAGE_SIZE = 100

CHARTS = [
    {"id": 3778678, "name": "热歌榜", "count": 200, "kind": "playlist"},
    {"id": 3779629, "name": "新歌榜", "count": 200, "kind": "playlist"},
    {"id": 19723756, "name": "飙升榜", "count": 200, "kind": "playlist"},
]


@WeapiCryptoRequest
def _daily_songs():
    return "/weapi/v1/discovery/recommend/songs", {}


@WeapiCryptoRequest
def _related_playlists(playlist_id: int):
    return "/weapi/discovery/simiPlaylist", {
        "songid": str(playlist_id),
        "limit": "6",
        "offset": "0",
    }


def song_item(song: dict) -> dict:
    return {
        "id": int(song.get("id") or 0),
        "title": song.get("name") or str(song.get("id") or ""),
        "artist": " / ".join(
            artist.get("name", "") for artist in song.get("ar") or song.get("artists") or []
        ),
        "duration_ms": int(song.get("dt") or song.get("duration") or 0),
    }


def song_page(
    songs: list[dict], kind: str, collection_id: int, offset: int, total: int | None = None
) -> dict:
    items = [item for song in songs if (item := song_item(song))["id"] > 0]
    return {
        "type": "songs",
        "kind": kind,
        "playlist_id": collection_id,
        "offset": offset,
        "total": int(total if total is not None else offset + len(items)),
        "items": items,
    }


SEARCH_TYPES = {
    "song": apis.cloudsearch.SONG,
    "album": apis.cloudsearch.ALBUM,
    "artist": apis.cloudsearch.ARTIST,
    "playlist": apis.cloudsearch.PLAYLIST,
    "mv": apis.cloudsearch.MV,
}


def search(keyword: str, offset: int = 0, kind: str = "song") -> dict:
    if kind == "auto":
        kind = "song"
    search_type = SEARCH_TYPES.get(kind, apis.cloudsearch.SONG)
    response = apis.cloudsearch.GetSearchResult(
        keyword, stype=search_type, limit=PAGE_SIZE, offset=offset
    )
    result = response.get("result") or {}
    if kind != "song":
        plural = "mvs" if kind == "mv" else f"{kind}s"
        items = []
        for item in result.get(plural) or []:
            items.append(
                {
                    "id": int(item.get("id") or 0),
                    "name": item.get("name") or "未命名",
                    "count": int(
                        item.get("trackCount")
                        or item.get("musicSize")
                        or item.get("albumSize")
                        or item.get("playCount")
                        or 0
                    ),
                    "kind": kind,
                }
            )
        return {"type": "playlists", "items": items}
    return song_page(
        result.get("songs") or [],
        f"search:{keyword}",
        0,
        offset,
        int(result.get("songCount") or 0),
    )


def daily(offset: int = 0) -> dict:
    response = _daily_songs()
    songs = (response.get("data") or {}).get("dailySongs") or []
    event = song_page(songs, "daily", 0, offset, len(songs))
    event.update(title="每日推荐", description="根据你的音乐口味每日更新")
    return event


def liked(playlists: list[dict], offset: int = 0) -> dict:
    playlist = next((item for item in playlists if item.get("specialType") == 5), None)
    if not playlist:
        return song_page([], "liked", 0, offset, 0)
    event = collection("playlist", int(playlist["id"]), offset)
    event["title"] = "我喜欢的音乐"
    return event


def collection(kind: str, collection_id: int, offset: int = 0) -> dict:
    if kind.startswith("search:"):
        return search(kind.removeprefix("search:"), offset)
    if kind == "album":
        response = apis.album.GetAlbumInfo(str(collection_id))
        album = response.get("album") or {}
        songs = album.get("songs") or response.get("songs") or []
        event = song_page(
            songs[offset : offset + PAGE_SIZE],
            kind,
            collection_id,
            offset,
            len(songs),
        )
        event.update(
            title=album.get("name") or "专辑",
            description=" / ".join(
                artist.get("name", "") for artist in album.get("artists") or []
            ),
        )
        return event
    if kind == "artist":
        response = apis.artist.GetArtistTracks(
            str(collection_id), offset=offset, limit=PAGE_SIZE
        )
        detail = apis.artist.GetArtistDetails(str(collection_id))
        artist = detail.get("data") or detail.get("artist") or {}
        event = song_page(
            response.get("songs") or [],
            kind,
            collection_id,
            offset,
            int(response.get("total") or 0),
        )
        event.update(
            title=artist.get("name") or "歌手",
            description=artist.get("briefDesc") or "",
        )
        return event
    if kind == "radio":
        response = radio.GetMoreRaidoContent(limit=PAGE_SIZE)
        songs = response.get("data") or []
        event = song_page(songs, kind, 0, offset, len(songs))
        event.update(title="私人 FM", description="持续推荐相似歌曲")
        return event
    if kind == "mv":
        return song_page([], kind, collection_id, offset, 0)
    info = apis.playlist.GetPlaylistInfo(collection_id, offset, True, PAGE_SIZE)
    playlist = info.get("playlist") or {}
    ids = playlist.get("trackIds") or []
    page = ids[offset : offset + PAGE_SIZE]
    response = apis.track.GetTrackDetail([item["id"] for item in page]) if page else {}
    event = song_page(
        response.get("songs") or [], "playlist", collection_id, offset, len(ids)
    )
    event.update(
        title=playlist.get("name") or "歌单",
        description=playlist.get("description") or "",
    )
    return event


def detail(kind: str, collection_id: int) -> dict:
    """Return a normalized detail page and related collections."""
    if kind == "mv":
        response = apis.video.GetMVDetail(str(collection_id))
        data = response.get("data") or response
        return {
            "type": "detail",
            "kind": kind,
            "id": collection_id,
            "title": data.get("name") or "MV",
            "description": data.get("desc") or "",
            "url": data.get("url") or "",
            "related": [],
        }
    event = collection(kind, collection_id)
    try:
        related = _related_playlists(collection_id) if kind == "playlist" else {}
    except Exception:
        related = {}
    event["type"] = "detail"
    event["id"] = collection_id
    event["related"] = [
        {
            "id": int(item.get("id") or 0),
            "name": item.get("name") or "相关歌单",
            "count": int(item.get("trackCount") or 0),
            "kind": "playlist",
        }
        for item in related.get("playlists") or []
    ]
    return event


def like(track_id: int, uid: int, like_value: bool = True) -> dict:
    response = apis.track.SetLikeTrack(track_id, like=like_value, userid=uid)
    code = int(response.get("code") or 0)
    if code not in {200, 0}:
        raise RuntimeError(f"favorite update rejected: code {code}")
    return {
        "type": "message",
        "message": "已收藏到我喜欢" if like_value else "已取消收藏",
    }


def user_collections(kind: str) -> dict:
    if kind == "charts":
        return {"type": "playlists", "items": CHARTS}
    if kind == "radio":
        return collection("radio", 0)
    if kind == "albums":
        response = apis.user.GetUserAlbumSubs(limit=100)
        source = response.get("data") or []
        items = [
            {
                "id": int(item.get("id") or 0),
                "name": item.get("name") or "未命名专辑",
                "count": int(item.get("size") or 0),
                "kind": "album",
            }
            for item in source
        ]
    elif kind == "artists":
        response = apis.user.GetUserArtistSubs(limit=100)
        source = response.get("data") or []
        items = [
            {
                "id": int(item.get("id") or 0),
                "name": item.get("name") or "未命名歌手",
                "count": int(item.get("musicSize") or item.get("albumSize") or 0),
                "kind": "artist",
            }
            for item in source
        ]
    else:
        raise ValueError(f"unknown collection kind: {kind}")
    return {"type": "playlists", "items": items}
