from __future__ import annotations

import json
import queue
import sys
import threading
from typing import TextIO


class JsonEmitter:
    """Serialize NDJSON on a dedicated thread so audio callbacks never block."""

    _STOP = object()

    def __init__(self, stream: TextIO | None = None) -> None:
        self.stream = stream or sys.stdout
        self.queue: queue.SimpleQueue = queue.SimpleQueue()
        self.thread = threading.Thread(target=self._run, name="ndjson-emitter", daemon=True)
        self.thread.start()

    def emit(self, event: dict) -> None:
        self.queue.put(event)

    def _run(self) -> None:
        while True:
            event = self.queue.get()
            if event is self._STOP:
                return
            self.stream.write(
                json.dumps(event, ensure_ascii=False, separators=(",", ":")) + "\n"
            )
            self.stream.flush()

    def close(self) -> None:
        self.queue.put(self._STOP)
        self.thread.join(timeout=5)
