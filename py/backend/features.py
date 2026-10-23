from __future__ import annotations

import math
from collections import deque

import numpy as np


class RhythmAnalyzer:
    def __init__(self, sample_rate: int = 44100, channels: int = 2) -> None:
        self.sample_rate = sample_rate
        self.channels = channels
        self.window = np.zeros(2048, dtype=np.float32)
        self.prev_mag = np.zeros(1025, dtype=np.float32)
        self.flux_history: deque[float] = deque(maxlen=28)
        self.onsets: deque[float] = deque(maxlen=8)
        self.last_time = 0.0
        self.hann = np.hanning(2048).astype(np.float32)

    def process(self, chunk, t: float) -> dict:
        samples = np.frombuffer(chunk, dtype=np.float32)
        if samples.size == 0:
            return self._empty(t)
        usable = samples.size - samples.size % self.channels
        frames = samples[:usable].reshape(-1, self.channels).mean(axis=1)
        zcr = float(np.mean(np.abs(np.diff(np.signbit(frames))))) if frames.size > 1 else 0.0
        if self.channels >= 2:
            stereo = samples[:usable].reshape(-1, self.channels)
            mid = stereo.mean(axis=1, keepdims=True)
            width = float(
                np.sqrt(
                    np.mean((stereo - mid) ** 2)
                    / max(np.mean(stereo**2), 1e-9)
                )
            )
        else:
            width = 0.0
        if frames.size >= self.window.size:
            self.window[:] = frames[-self.window.size :]
        else:
            self.window[:-frames.size] = self.window[frames.size :]
            self.window[-frames.size :] = frames
        rms = float(np.sqrt(np.mean(frames * frames) + 1e-12))
        peak = float(np.max(np.abs(frames)) + 1e-12)
        crest = min(peak / max(rms, 1e-9), 4.0) / 4.0
        mag = np.abs(np.fft.rfft(self.window * self.hann))
        freqs = np.fft.rfftfreq(self.window.size, 1.0 / self.sample_rate)
        bass = self._band(mag, freqs, 25, 250)
        mid = self._band(mag, freqs, 250, 2200)
        treble = self._band(mag, freqs, 2200, 10000)
        flux_raw = float(np.maximum(mag - self.prev_mag, 0.0).sum() / mag.size)
        self.prev_mag = mag
        mean = float(np.mean(self.flux_history)) if self.flux_history else 0.02
        std = float(np.std(self.flux_history)) if self.flux_history else 0.02
        onset = flux_raw > mean + max(std * 1.4, 0.012)
        if onset:
            self.onsets.append(t)
        self.flux_history.append(flux_raw)
        self.last_time = t
        flux = min(flux_raw / 1.5, 1.0)
        bpm = self._bpm()
        spectrum = self._spectrum(mag, freqs)
        total = max(float(mag.sum()), 1e-9)
        centroid = float((freqs * mag).sum() / (total * max(self.sample_rate / 2, 1)))
        spread = float(
            np.sqrt((((freqs / max(self.sample_rate / 2, 1)) - centroid) ** 2 * mag).sum() / total)
        )
        flatness = float(np.exp(np.mean(np.log(mag + 1e-9))) / max(np.mean(mag), 1e-9))
        cumulative = np.cumsum(mag)
        rolloff_idx = int(np.searchsorted(cumulative, total * 0.85))
        rolloff = min(rolloff_idx / max(len(freqs) - 1, 1), 1.0)
        return {
            "t": round(t, 3),
            "rms": round(min(math.log1p(rms * 8.0) / 1.8, 1.0), 4),
            "bass": round(self._scaled(bass), 4),
            "mid": round(self._scaled(mid), 4),
            "treble": round(self._scaled(treble), 4),
            "flux": round(flux, 4),
            "onset": 1.0 if onset else 0.0,
            "bpm": round(bpm, 1),
            "spectrum": [round(v, 4) for v in spectrum],
            "centroid": round(min(max(centroid, 0.0), 1.0), 4),
            "spread": round(min(max(spread, 0.0), 1.0), 4),
            "flatness": round(min(max(flatness, 0.0), 1.0), 4),
            "rolloff": round(rolloff, 4),
            "zcr": round(min(max(zcr, 0.0), 1.0), 4),
            "crest": round(min(max(crest, 0.0), 1.0), 4),
            "stereo_width": round(min(max(width, 0.0), 1.0), 4),
        }

    def _band(self, mag: np.ndarray, freqs: np.ndarray, lo: float, hi: float) -> float:
        mask = (freqs >= lo) & (freqs < hi)
        return float(np.sqrt(np.mean(mag[mask] ** 2) + 1e-12))

    @staticmethod
    def _scaled(value: float) -> float:
        return min(math.sqrt(max(value, 0.0) / 180.0), 1.0)

    def _bpm(self) -> float:
        if len(self.onsets) < 3:
            return 0.0
        intervals = np.diff(np.fromiter(self.onsets, dtype=np.float64))
        intervals = intervals[intervals > 0.18]
        if intervals.size == 0:
            return 0.0
        bpm = 60.0 / float(np.median(intervals))
        while bpm < 60.0:
            bpm *= 2.0
        while bpm > 190.0:
            bpm /= 2.0
        return bpm

    @staticmethod
    def _spectrum(mag: np.ndarray, freqs: np.ndarray) -> list[float]:
        low = 40.0
        ratio = (12000.0 / low) ** (1.0 / 16.0)
        out = []
        for i in range(16):
            lo = low * ratio**i
            hi = lo * ratio
            mask = (freqs >= lo) & (freqs < hi)
            value = float(np.sqrt(np.mean(mag[mask] ** 2) + 1e-12))
            out.append(min((value / 180.0) ** 0.5, 1.0))
        return out

    def _empty(self, t: float) -> dict:
        return {
            "t": round(t, 3),
            "rms": 0.0,
            "bass": 0.0,
            "mid": 0.0,
            "treble": 0.0,
            "flux": 0.0,
            "onset": 0.0,
            "bpm": 0.0,
            "spectrum": [0.0] * 16,
            "centroid": 0.0,
            "spread": 0.0,
            "flatness": 0.0,
            "rolloff": 0.0,
            "zcr": 0.0,
            "crest": 0.0,
            "stereo_width": 0.0,
        }
