from __future__ import annotations

import os
import queue
import socket
import struct
import threading


class PcmSender:
    """Send decoded PCM to Rust's SIMD analyzer without blocking audio callbacks."""

    _STOP = object()

    def __init__(self, path: str, sample_rate: int, channels: int = 2) -> None:
        self.socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.socket.connect(path)
        self.socket.sendall(struct.pack("<4sIHH", b"HBP1", sample_rate, channels, 0))
        self.queue: queue.SimpleQueue = queue.SimpleQueue()
        self.thread = threading.Thread(target=self._run, name="pcm-sender", daemon=True)
        self.thread.start()

    @classmethod
    def from_env(cls, sample_rate: int) -> "PcmSender | None":
        path = os.getenv("HAPPYBIRTHDAY_PCM_SOCKET")
        if not path:
            return None
        try:
            return cls(path, sample_rate)
        except OSError:
            return None

    def send(self, chunk, timestamp: float) -> None:
        payload = memoryview(chunk).tobytes()
        self.queue.put(struct.pack("<fI", timestamp, len(payload)) + payload)

    def _run(self) -> None:
        while True:
            payload = self.queue.get()
            if payload is self._STOP:
                return
            try:
                self.socket.sendall(payload)
            except OSError:
                return

    def close(self) -> None:
        self.queue.put(self._STOP)
        self.thread.join(timeout=2)
        self.socket.close()
