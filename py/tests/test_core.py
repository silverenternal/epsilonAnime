import unittest

import numpy as np

from backend.features import RhythmAnalyzer
from backend.lyrics import locate, parse_lyrics, parse_rich_lyrics
from backend.ncm import parse_song_id


class CoreTests(unittest.TestCase):
    def test_song_id_from_url(self):
        self.assertEqual(
            parse_song_id("https://music.163.com/song?id=3347119767&uct2=x"),
            3347119767,
        )
        self.assertEqual(parse_song_id("https://music.163.com/song/42"), 42)

    def test_lyrics_and_locate(self):
        lines = parse_lyrics(
            '[00:01.00]one\n[00:03.50]two\n{"t":-1,"c":[{"tx":"meta"}]}',
            "[00:03.50]zwei",
        )
        self.assertEqual(len(lines), 2)
        self.assertEqual(lines[1].translation, "zwei")
        idx, progress = locate(lines, 2.25)
        self.assertEqual(idx, 0)
        self.assertGreater(progress, 0.0)

    def test_features_are_bounded(self):
        analyzer = RhythmAnalyzer()
        t = 0.0
        for _ in range(20):
            samples = np.sin(np.arange(1024) * 0.05).astype(np.float32)
            stereo = np.repeat(samples, 2)
            event = analyzer.process(stereo, t)
            for key in ("rms", "bass", "mid", "treble", "flux"):
                self.assertGreaterEqual(event[key], 0.0)
                self.assertLessEqual(event[key], 1.0)
            t += 1024 / 44100
        self.assertGreater(event["rms"], 0.0)

    def test_enhanced_lyrics_keep_segments_and_romaji(self):
        lines = parse_rich_lyrics(
            "[00:01.00]你好",
            romanization="[00:01.00]ni hao",
            karaoke="[00:01.00]<0,200,0>你<200,300,0>好",
        )
        self.assertEqual(lines[0].romaji, "ni hao")
        self.assertEqual(len(lines[0].segments), 2)
        self.assertAlmostEqual(lines[0].segments[1].start, 1.2)


if __name__ == "__main__":
    unittest.main()
