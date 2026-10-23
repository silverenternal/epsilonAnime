import os
import struct
import unittest

import numpy as np

from backend.cover import _write_particles


class CoverParticleTests(unittest.TestCase):
    def test_opencv_particle_extraction(self):
        y, x = np.mgrid[0:96, 0:96]
        rgb = np.stack((x * 2 + 30, y * 2 + 20, (x + y)), axis=-1).astype(np.uint8)
        path, count = _write_particles(rgb)
        try:
            with open(path, "rb") as handle:
                header = handle.read(16)
                body = handle.read()
            magic, _version, _flags, stored_count, width, height = struct.unpack(
                "<4sHHIHH", header
            )
            self.assertEqual(magic, b"HBPC")
            self.assertEqual((width, height), (512, 512))
            self.assertEqual(stored_count, count)
            self.assertGreater(count, 1000)
            self.assertEqual(len(body), count * 10)
            first = struct.unpack("<HHBBBBBB", body[:10])
            self.assertTrue(all(0 <= value <= 255 for value in first[2:]))
        finally:
            os.remove(path)


if __name__ == "__main__":
    unittest.main()
