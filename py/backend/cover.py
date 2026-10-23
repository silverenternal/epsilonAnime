from __future__ import annotations

import atexit
import io
import os
import struct
import tempfile
from dataclasses import dataclass

import cv2
import numpy as np
import requests
from PIL import Image, ImageOps


@dataclass
class CoverInfo:
    path: str
    cols: int
    rows: int
    particle_path: str
    particle_count: int


def materialize(url: str, size: int = 512, cleanup: bool = True) -> CoverInfo:
    if not url:
        raise RuntimeError("track has no cover URL")
    response = requests.get(url, timeout=20)
    response.raise_for_status()
    image = Image.open(io.BytesIO(response.content))
    image = ImageOps.exif_transpose(image).convert("RGB")
    side = min(image.size)
    left = (image.width - side) // 2
    top = (image.height - side) // 2
    image = image.crop((left, top, left + side, top + side))
    rgb = np.asarray(image, dtype=np.uint8)
    resized = cv2.resize(rgb, (size, size), interpolation=cv2.INTER_AREA)
    resized = np.ascontiguousarray(resized)
    path = os.path.join(
        tempfile.gettempdir(), f"happybirthday-cover-{os.getpid()}.rgb"
    )
    with open(path, "wb") as handle:
        handle.write(resized.tobytes())
    particle_path, particle_count = _write_particles(resized)
    if cleanup:
        atexit.register(_safe_remove, path)
        atexit.register(_safe_remove, particle_path)
    return CoverInfo(
        path=path,
        cols=size,
        rows=size,
        particle_path=particle_path,
        particle_count=particle_count,
    )


def _write_particles(rgb: np.ndarray) -> tuple[str, int]:
    work_size = 512
    work = cv2.resize(rgb, (work_size, work_size), interpolation=cv2.INTER_AREA)
    gray = cv2.cvtColor(work, cv2.COLOR_RGB2GRAY)
    gray = cv2.createCLAHE(clipLimit=2.2, tileGridSize=(8, 8)).apply(gray)
    gx = cv2.Sobel(gray, cv2.CV_32F, 1, 0, ksize=3)
    gy = cv2.Sobel(gray, cv2.CV_32F, 0, 1, ksize=3)
    edge = np.clip(np.hypot(gx, gy) / 420.0, 0.0, 1.0)
    particles = []
    block = 2
    for y in range(0, work_size, block):
        for x in range(0, work_size, block):
            patch = work[y : y + block, x : x + block].reshape(-1, 3).mean(axis=0)
            lum = float(gray[y : y + block, x : x + block].mean()) / 255.0
            edge_v = float(edge[y : y + block, x : x + block].max())
            if lum < 0.008 and edge_v < 0.012:
                continue
            salience = min(0.12 + lum * 0.55 + edge_v * 0.85, 1.0)
            particles.append(
                (
                    min(x + 1, 65535),
                    min(y + 1, 65535),
                    int(patch[0]),
                    int(patch[1]),
                    int(patch[2]),
                    int(lum * 255),
                    int(edge_v * 255),
                    int(salience * 255),
                )
            )
            if edge_v > 0.12:
                particles.append(
                    (
                        min(x + (1 if (x + y) % 4 else 0), 65535),
                        min(y + (1 if (x + y) % 6 else 0), 65535),
                        int(patch[0]),
                        int(patch[1]),
                        int(patch[2]),
                        int(lum * 255),
                        int(edge_v * 255),
                        int(min(salience + 0.15, 1.0) * 255),
                    )
                )
    path = os.path.join(
        tempfile.gettempdir(), f"happybirthday-particles-{os.getpid()}.hbp"
    )
    with open(path, "wb") as handle:
        handle.write(
            struct.pack("<4sHHIHH", b"HBPC", 1, 0, len(particles), work_size, work_size)
        )
        for particle in particles:
            handle.write(struct.pack("<HHBBBBBB", *particle))
    return path, len(particles)


def _safe_remove(path: str) -> None:
    try:
        os.remove(path)
    except OSError:
        pass


def register_cleanup(path: str) -> None:
    atexit.register(_safe_remove, path)
