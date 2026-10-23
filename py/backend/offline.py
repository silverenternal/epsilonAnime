from __future__ import annotations

import json
import time
from pathlib import Path

PATH = Path(__file__).resolve().parents[2] / "target" / "offline" / "library.json"
MAX_ENTRIES = 30


def load_all() -> dict:
    try:
        payload = json.loads(PATH.read_text(encoding="utf-8"))
        return payload if isinstance(payload, dict) else {}
    except (OSError, ValueError):
        return {}


def store(key: str, event: dict) -> None:
    payload = load_all()
    payload[key] = {"cached_at": time.time(), "event": event}
    entries = [name for name in payload if not name.startswith("_")]
    while len(entries) > MAX_ENTRIES:
        oldest = min(entries, key=lambda item: payload[item].get("cached_at", 0))
        payload.pop(oldest, None)
        entries.remove(oldest)
    history = payload.setdefault("_history", [])
    summary = {
        "key": key,
        "at": time.time(),
        "type": event.get("type", ""),
        "kind": event.get("kind", ""),
        "title": event.get("title")
        or event.get("name")
        or " / ".join(
            item.get("title", "") for item in (event.get("items") or [])[:2]
        ),
    }
    history.append(summary)
    payload["_history"] = history[-MAX_ENTRIES:]
    PATH.parent.mkdir(parents=True, exist_ok=True)
    temp = PATH.with_suffix(".tmp")
    temp.write_text(json.dumps(payload, ensure_ascii=False), encoding="utf-8")
    temp.replace(PATH)


def load(key: str) -> dict | None:
    item = load_all().get(key) or {}
    event = item.get("event")
    return event if isinstance(event, dict) else None


def history() -> list[dict]:
    items = load_all().get("_history") or []
    return [item for item in items if isinstance(item, dict)][-MAX_ENTRIES:]


def clear() -> int:
    payload = load_all()
    count = len([key for key in payload if not key.startswith("_")])
    try:
        PATH.unlink()
    except FileNotFoundError:
        pass
    return count


def record_recent(song: dict) -> None:
    payload = load_all()
    recent = [
        item
        for item in payload.get("_recent") or []
        if int(item.get("id") or 0) != int(song.get("id") or 0)
    ]
    recent.insert(0, song)
    payload["_recent"] = recent[:50]
    PATH.parent.mkdir(parents=True, exist_ok=True)
    temp = PATH.with_suffix(".tmp")
    temp.write_text(json.dumps(payload, ensure_ascii=False), encoding="utf-8")
    temp.replace(PATH)


def recent() -> list[dict]:
    return [
        item
        for item in load_all().get("_recent") or []
        if isinstance(item, dict)
    ]
