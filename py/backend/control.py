from __future__ import annotations

import json
import queue
import sys
import threading
import time
from typing import TextIO


class ControlBus:
    """Read JSON control commands from stdin without blocking playback."""

    def __init__(self, stream: TextIO | None = None) -> None:
        self.stream = stream or sys.stdin
        self.queue: queue.SimpleQueue = queue.SimpleQueue()
        self.closed = threading.Event()
        self.thread = threading.Thread(target=self._run, name="control-bus", daemon=True)

    def start(self) -> None:
        self.thread.start()

    def _run(self) -> None:
        try:
            for line in self.stream:
                try:
                    self.queue.put(json.loads(line))
                except json.JSONDecodeError:
                    continue
        finally:
            self.closed.set()

    def drain(self) -> list[dict]:
        out = []
        while True:
            try:
                out.append(self.queue.get_nowait())
            except queue.Empty:
                return out

    def wait_for(self, command: str, timeout: float = 60.0) -> dict | None:
        deadline = time.monotonic() + timeout
        deferred = []
        try:
            while time.monotonic() < deadline:
                try:
                    item = self.queue.get(timeout=0.1)
                except queue.Empty:
                    if self.closed.is_set():
                        return None
                    continue
                if item.get("cmd") == command:
                    return item
                deferred.append(item)
            return None
        finally:
            for item in deferred:
                self.queue.put(item)
