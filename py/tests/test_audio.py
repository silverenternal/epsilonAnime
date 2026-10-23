import unittest
from unittest.mock import Mock, patch

import miniaudio

from backend.ncm import Track, _audio_item, _audio_type, refresh_track_audio
from backend.player import (
    HttpSource,
    _apply_volume,
    _open_stream,
    playback_device_id,
    playback_backends,
    preload_stream,
    replay_gain_factor,
)


def track() -> Track:
    return Track(
        song_id=123,
        title="title",
        artist="artist",
        duration_ms=1000,
        cover_url="cover",
        lyric_text="",
        translation_text="",
        audio_url="https://example.test/song.m4a?token=old",
        audio_type="m4a",
        gain=0.0,
        peak=1.0,
    )


class AudioTests(unittest.TestCase):
    def test_audio_type_uses_url_fallback(self):
        self.assertEqual(
            _audio_type({"type": "m4a"}, "https://example.test/song.mp3?token=x"),
            "mp3",
        )
        with self.assertRaises(RuntimeError):
            _audio_type({"type": "m4a"}, "https://example.test/song.m4a")

    @patch("backend.ncm._session")
    @patch("backend.ncm._audio_item")
    def test_refresh_preserves_metadata(self, audio_item, session):
        audio_item.return_value = {
            "url": "https://example.test/fresh.mp3?token=new",
            "type": "mp3",
            "gain": 1.0,
            "peak": 0.8,
        }
        old = track()
        fresh = refresh_track_audio(old)
        self.assertEqual(fresh.song_id, old.song_id)
        self.assertEqual(fresh.lyric_text, old.lyric_text)
        self.assertEqual(fresh.audio_url, "https://example.test/fresh.mp3?token=new")
        self.assertEqual(fresh.audio_type, "mp3")
        session.assert_called_once_with(None)

    @patch("backend.ncm.apis.track.GetTrackAudio")
    @patch("backend.ncm.apis.track.GetTrackAudioV1")
    def test_audio_api_falls_back_to_compatible_encode(self, current, legacy):
        current.return_value = {"data": [{"url": "", "type": "m4a"}]}
        legacy.return_value = {
            "data": [{"url": "https://example.test/fallback.mp3", "type": "mp3"}]
        }
        item = _audio_item(123)
        self.assertEqual(item["type"], "mp3")
        legacy.assert_called_once_with([123], bitrate=128000, encodeType="mp3")

    @patch("backend.player.miniaudio.stream_any")
    @patch("backend.player.requests.get")
    def test_network_source_is_returned_as_decoder_keepalive(self, get, stream_any):
        response = Mock()
        response.headers = {"Content-Length": "3"}
        response.raise_for_status.return_value = None
        get.return_value = response
        stream_any.return_value = object()
        current = track()
        current.audio_type = "mp3"
        source, _stream, _seeked = _open_stream(current, 44100)
        self.assertIsInstance(source, HttpSource)
        self.assertIs(source.response, response)

    def test_volume_scales_pcm_samples(self):
        import numpy as np

        source = np.array([0.5, -0.5, 1.0], dtype=np.float32)
        output = np.frombuffer(_apply_volume(source.tobytes(), 0.5), dtype=np.float32)
        np.testing.assert_allclose(output, [0.25, -0.25, 0.5])

    @patch("backend.player.requests.get")
    def test_range_seek_uses_total_length_from_content_range(self, get):
        first = Mock(
            status_code=200,
            headers={"Content-Range": "bytes 0-99/1000", "Content-Length": "100"},
        )
        second = Mock(
            status_code=206,
            headers={"Content-Range": "bytes 900-999/1000", "Content-Length": "100"},
        )
        get.return_value = second
        source = HttpSource("https://example.test/song.mp3", first)
        source.seek(-100, miniaudio.SeekOrigin.END)
        self.assertEqual(source.total_bytes, 1000)
        self.assertEqual(
            get.call_args.kwargs["headers"]["Range"], "bytes=900-"
        )

    @patch("backend.player.requests.get")
    def test_seek_to_eof_does_not_request_an_unsatisfiable_range(self, get):
        response = Mock(
            status_code=200,
            headers={"Content-Range": "bytes 0-99/100", "Content-Length": "100"},
        )
        source = HttpSource("https://example.test/song.mp3", response)
        self.assertTrue(source.seek(0, miniaudio.SeekOrigin.END))
        self.assertEqual(source.read(32), b"")
        get.assert_not_called()

    @patch("backend.player._open_stream")
    def test_preload_returns_decoder_before_playback(self, open_stream):
        open_stream.return_value = ("source", "stream", False)
        self.assertEqual(preload_stream(track()), ("source", "stream", False))

    def test_replay_gain_is_bounded_and_peak_safe(self):
        self.assertAlmostEqual(replay_gain_factor(0.0, 1.0), 1.0)
        self.assertLessEqual(replay_gain_factor(24.0, 0.001), 4.0)
        self.assertGreater(replay_gain_factor(-24.0, 1.0), 0.0)

    @patch.dict("os.environ", {}, clear=True)
    def test_default_output_device_is_optional(self):
        self.assertEqual(playback_device_id(), (None, ""))

    @patch.dict("os.environ", {"EPSILON_ANIME_AUDIO_BACKEND": "null"})
    def test_null_backend_is_available_for_headless_playback_checks(self):
        self.assertEqual(playback_backends(), [miniaudio.Backend.NULL])


if __name__ == "__main__":
    unittest.main()
