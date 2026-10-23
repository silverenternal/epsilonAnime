import random
import string
import unittest

from backend.lyrics import locate, parse_rich_lyrics


class ProtocolPropertyTests(unittest.TestCase):
    def test_arbitrary_lyrics_never_raise(self):
        rng = random.Random(20261024)
        alphabet = string.printable + "你好こんにちは"
        for _ in range(200):
            payload = "".join(rng.choice(alphabet) for _ in range(rng.randrange(0, 300)))
            lines = parse_rich_lyrics(payload, payload, payload, payload)
            for timestamp in (-10.0, 0.0, 1.5, 9999.0):
                index, progress = locate(lines, timestamp)
                self.assertGreaterEqual(index, 0)
                self.assertGreaterEqual(progress, 0.0)
                self.assertLessEqual(progress, 1.0)


if __name__ == "__main__":
    unittest.main()
