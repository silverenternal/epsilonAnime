from __future__ import annotations

import sys
import threading
import time
import os
from typing import Callable

import miniaudio
import numpy as np
import requests

from .features import RhythmAnalyzer
from .lyrics import LyricLine, locate
from .ncm import Track, refresh_track_audio
from .pcm import PcmSender


class HttpSource(miniaudio.StreamableSource):
    def __init__(self, url: str, response: requests.Response) -> None:
        self.url = url
        self.response = response
        self.headers = {
            "User-Agent": "Mozilla/5.0 happybirthday",
            "Referer": "https://music.163.com/",
        }
        self.total_bytes = _response_length(response)
        self.position = 0
        self.at_end = False

    def read(self, num_bytes: int):
        if self.at_end:
            return b""
        data = self.response.raw.read(num_bytes)
        self.position += len(data)
        if not data:
            self.at_end = True
        return data

    def close(self) -> None:
        self.response.close()

    def seek(self, offset: int, origin):
        if origin == miniaudio.SeekOrigin.START:
            target = offset
        elif origin == miniaudio.SeekOrigin.CURRENT:
            target = self.position + offset
        else:
            target = max(self.total_bytes + offset, 0)
        target = min(max(target, 0), self.total_bytes)
        if target == self.position:
            self.at_end = target >= self.total_bytes
            return True
        if target >= self.total_bytes:
            self.position = self.total_bytes
            self.at_end = True
            return True
        self.response.close()
        headers = {**self.headers, "Range": f"bytes={max(target, 0)}-"}
        response = requests.get(self.url, stream=True, timeout=(15, 60), headers=headers)
        response.raise_for_status()
        self.response = response
        self.total_bytes = _response_length(response, self.total_bytes)
        if self.response.status_code != 206:
            raise OSError("HTTP range seek is not supported")
        self.position = target
        self.at_end = False
        return True


def _response_length(response: requests.Response, fallback: int = 0) -> int:
    content_range = response.headers.get("Content-Range") or ""
    if not isinstance(content_range, str):
        content_range = ""
    if "/" in content_range:
        total = content_range.rsplit("/", 1)[-1]
        if total.isdigit():
            return int(total)
    length = response.headers.get("Content-Length")
    return int(length) if isinstance(length, str) and length.isdigit() else fallback


def playback_device_id():
    requested = os.getenv("HAPPYBIRTHDAY_AUDIO_DEVICE", "").strip()
    if not requested:
        return None, ""
    try:
        for device in miniaudio.Devices().get_playbacks():
            if requested.lower() in str(device.get("name", "")).lower():
                return device.get("id"), str(device.get("name", requested))
    except Exception as exc:
        print(f"audio: device enumeration failed: {exc}", file=sys.stderr)
    return None, requested


def playback_backends():
    backend = os.getenv("HAPPYBIRTHDAY_AUDIO_BACKEND", "").strip().lower()
    if backend == "null":
        return [miniaudio.Backend.NULL]
    return []


FORMATS = {
    "mp3": miniaudio.FileFormat.MP3,
    "flac": miniaudio.FileFormat.FLAC,
    "wav": miniaudio.FileFormat.WAV,
    "ogg": miniaudio.FileFormat.VORBIS,
}


def replay_gain_factor(gain_db: float, peak: float) -> float:
    linear = 10.0 ** (max(gain_db, -24.0) / 20.0)
    return min(linear / max(peak, 0.01), 4.0)


class SeekRequest(Exception):
    def __init__(self, seconds: float) -> None:
        super().__init__(f"seek to {seconds:.2f}s")
        self.seconds = max(seconds, 0.0)


def _open_stream(track: Track, sample_rate: int, seek_seconds: float = 0.0):
    fmt = FORMATS.get(track.audio_type)
    if fmt is None:
        raise RuntimeError(f"unsupported audio type: {track.audio_type}")
    response = requests.get(
        track.audio_url,
        stream=True,
        timeout=(15, 60),
        headers={"User-Agent": "Mozilla/5.0 happybirthday", "Referer": "https://music.163.com/"},
    )
    response.raise_for_status()
    source = HttpSource(track.audio_url, response)
    try:
        stream = miniaudio.stream_any(
            source,
            source_format=fmt,
            output_format=miniaudio.SampleFormat.FLOAT32,
            nchannels=2,
            sample_rate=sample_rate,
            frames_to_read=1024,
            seek_frame=int(seek_seconds * sample_rate),
        )
        return source, stream, seek_seconds > 0
    except Exception:
        source.close()
        if seek_seconds > 0:
            return _open_stream(track, sample_rate, 0.0)
        raise
    # miniaudio stores a C handle to this object. Keep the source itself alive
    # for the entire decoder lifetime; a cdata handle alone does not own it.
    return source, stream


def preload_stream(track: Track, sample_rate: int = 44100):
    """Resolve and open the first decoder chunk before the track is scheduled."""
    return _open_stream(track, sample_rate, 0.0)


def play(
    track: Track,
    lines: list[LyricLine],
    emit: Callable[[dict], None],
    analyze_only: bool = False,
    max_seconds: float | None = None,
    sample_rate: int = 44100,
    control=None,
    level: str = "standard",
    cookie: str | None = None,
    prepared_stream=None,
) -> None:
    start_seconds = 0.0
    state = {
        "volume": 1.0,
        "muted": False,
        "last_volume": 1.0,
        "lyric_offset": 0.0,
    }
    while True:
        if prepared_stream is not None:
            source, stream, decoder_seek = prepared_stream
            prepared_stream = None
        else:
            try:
                source, stream, decoder_seek = _open_stream(
                    track, sample_rate, start_seconds
                )
            except Exception as first_error:
                print(f"audio: refreshing expired stream after {first_error}", file=sys.stderr)
                track = refresh_track_audio(track, level=level, cookie=cookie)
                source, stream, decoder_seek = _open_stream(
                    track, sample_rate, start_seconds
                )
        pcm = PcmSender.from_env(sample_rate)
        analyzer = None if pcm else RhythmAnalyzer(sample_rate=sample_rate, channels=2)
        try:
            if analyze_only:
                _analyze_stream(stream, lines, analyzer, pcm, emit, sample_rate, max_seconds)
                return
            _play_stream(
                stream,
                lines,
                analyzer,
                pcm,
                emit,
                sample_rate,
                track.duration_ms / 1000.0,
                control,
                start_seconds,
                state,
                track.gain,
                track.peak,
                decoder_seek,
            )
            return
        except SeekRequest as request:
            start_seconds = min(request.seconds, track.duration_ms / 1000.0)
            emit({"type": "state", "state": "seeking", "position": start_seconds})
        finally:
            if pcm:
                pcm.close()
            try:
                stream.close()
            except Exception:
                pass
            source.close()


def _analyze_stream(stream, lines, analyzer, pcm, emit, sample_rate, max_seconds) -> None:
    processed = 0
    last_lyric = -1.0
    for index, chunk in enumerate(stream):
        t = processed / sample_rate
        if max_seconds is not None and t >= max_seconds:
            break
        _emit_audio(chunk, t, analyzer, pcm, emit, index)
        processed += len(chunk) // 2
        last_lyric = _emit_lyric(lines, processed / sample_rate, emit, last_lyric)


def _play_stream(
    stream,
    lines,
    analyzer,
    pcm,
    emit,
    sample_rate,
    duration,
    control,
    start_seconds,
    state,
    replay_gain,
    replay_peak,
    decoder_seek,
) -> None:
    done = threading.Event()
    processed = [int(start_seconds * sample_rate)]
    played = [0]
    lyric = [-1.0]
    chunks = [0]
    gain = replay_gain_factor(replay_gain, replay_peak)
    print(
        f"audio: starting miniaudio playback gain={gain:.3f}",
        file=sys.stderr,
        flush=True,
    )

    def process(chunk):
        t = processed[0] / sample_rate
        output = _apply_volume(chunk, state["volume"] * gain)
        _emit_audio(output, t, analyzer, pcm, emit, chunks[0])
        processed[0] += len(chunk) // 2
        chunks[0] += 1
        return output

    def progress(frames_played: int):
        played[0] += max(frames_played, 0)
        t = max(
            start_seconds + (played[0] - int(sample_rate * 0.08)) / sample_rate,
            0,
        )
        lyric[0] = _emit_lyric(
            lines, t, emit, lyric[0], state["lyric_offset"]
        )

    def finished():
        done.set()

    target_frames = processed[0]
    remaining_skip = 0 if decoder_seek else target_frames
    while remaining_skip > 0:
        try:
            skipped = next(stream)
        except StopIteration:
            return
        remaining_skip -= len(skipped) // 2
    processed[0] = target_frames
    wrapped = miniaudio.stream_with_callbacks(
        stream, progress_callback=progress, frame_process_method=process, end_callback=finished
    )
    next(wrapped)
    try:
        device_id, device_name = playback_device_id()
        backends = playback_backends()
        device = miniaudio.PlaybackDevice(
            output_format=miniaudio.SampleFormat.FLOAT32,
            nchannels=2,
            sample_rate=sample_rate,
            buffersize_msec=160,
            device_id=device_id,
            backends=backends,
            app_name="happybirthday",
        )
    except Exception as exc:
        print(f"audio: no output device, analyze-only fallback: {exc}", file=sys.stderr)
        _analyze_stream(stream, lines, analyzer, pcm, emit, sample_rate, None)
        return
    emit(
        {
            "type": "state",
            "state": "audio_device",
            "name": device_name or ("null" if backends else "default"),
        }
    )
    print(f"audio: output device {device_name or 'default'}", file=sys.stderr, flush=True)
    try:
        pending_seek = None
        with device:
            device.start(wrapped)
            deadline = time.monotonic() + max(duration + 45.0, 60.0)
            paused = False
            last_reported = -1.0
            while (
                not done.is_set()
                and time.monotonic() < deadline
                and not (control and control.closed.is_set())
            ):
                if control:
                    for command in control.drain():
                        action = command.get("cmd")
                        if action == "pause" and not paused:
                            device.stop()
                            paused = True
                            emit({"type": "state", "state": "paused"})
                        elif action == "resume" and paused:
                            device.start(wrapped)
                            paused = False
                            emit({"type": "state", "state": "playing"})
                        elif action == "volume":
                            value = float(command.get("value", 1.0))
                            state["volume"] = max(0.0, min(value, 1.5))
                            state["muted"] = False
                            emit(
                                {
                                    "type": "state",
                                    "state": "volume",
                                    "volume": state["volume"],
                                }
                            )
                        elif action == "mute":
                            muted = bool(command.get("value", True))
                            if muted and not state["muted"]:
                                state["last_volume"] = state["volume"]
                                state["volume"] = 0.0
                            elif not muted and state["muted"]:
                                state["volume"] = state["last_volume"]
                            state["muted"] = muted
                            emit(
                                {
                                    "type": "state",
                                    "state": "mute",
                                    "muted": muted,
                                    "volume": state["volume"],
                                }
                            )
                        elif action == "seek":
                            pending_seek = max(float(command.get("seconds", 0.0)), 0.0)
                            break
                        elif action == "lyric_offset":
                            state["lyric_offset"] = max(
                                -10.0,
                                min(float(command.get("seconds", 0.0)), 10.0),
                            )
                            lyric[0] = -1.0
                            emit(
                                {
                                    "type": "state",
                                    "state": "lyric_offset",
                                    "offset": state["lyric_offset"],
                                }
                            )
                if pending_seek is not None:
                    break
                position = start_seconds + played[0] / sample_rate
                if position - last_reported >= 0.5:
                    last_reported = position
                    emit(
                        {
                            "type": "playback",
                            "position": round(position, 3),
                            "volume": round(state["volume"], 3),
                            "muted": state["muted"],
                            "paused": paused,
                        }
                    )
                time.sleep(0.02)
        if pending_seek is not None:
            raise SeekRequest(pending_seek)
    except SeekRequest:
        raise
    except Exception as exc:
        raise RuntimeError(f"audio playback failed: {exc}") from exc


def _emit_audio(chunk, t, analyzer, pcm, emit, index) -> None:
    if pcm:
        pcm.send(chunk, t)
        return
    if index % 2 != 0:
        return
    if analyzer is None:
        return
    try:
        event = {"type": "features", **analyzer.process(chunk, t)}
        emit(event)
    except Exception as exc:
        print(f"analysis warning: {exc}", file=sys.stderr, flush=True)


def _apply_volume(chunk, volume: float):
    if abs(volume - 1.0) < 1.0e-4:
        return chunk
    samples = np.frombuffer(chunk, dtype=np.float32)
    return np.clip(samples * volume, -1.0, 1.0).astype(np.float32).tobytes()


def _emit_lyric(lines, t, emit, last_t: float, offset: float = 0.0) -> float:
    if not lines or t - last_t < 0.1:
        return last_t
    index, progress = locate(lines, t + offset)
    line = lines[index]
    emit(
        {
            "type": "lyric",
            "index": index,
            "text": line.text,
            "translation": line.translation,
            "romaji": line.romaji,
            "segments": [
                {"start": segment.start, "end": segment.end, "text": segment.text}
                for segment in line.segments
            ],
            "progress": round(progress, 4),
            "offset": round(offset, 3),
        }
    )
    return t
