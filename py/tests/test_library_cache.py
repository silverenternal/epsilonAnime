import json
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

from backend.cache import prune_cache, scrub_persisted_urls
from backend import offline
from backend.auth import _cached_emit, parse_collection_value
from backend.library_api import collection, daily, search


class LibraryAndCacheTests(unittest.TestCase):
    def test_search_maps_page_and_total(self):
        response = {
            "result": {
                "songCount": 240,
                "songs": [
                    {
                        "id": 7,
                        "name": "song",
                        "ar": [{"name": "artist"}],
                        "dt": 1000,
                    }
                ],
            }
        }
        with patch("backend.library_api.apis.cloudsearch.GetSearchResult", return_value=response):
            event = search("needle", offset=100)
        self.assertEqual(event["offset"], 100)
        self.assertEqual(event["total"], 240)
        self.assertEqual(event["items"][0]["id"], 7)

    def test_search_maps_non_song_facets(self):
        response = {
            "result": {
                "albums": [{"id": 8, "name": "album", "musicSize": 12}],
            }
        }
        with patch("backend.library_api.apis.cloudsearch.GetSearchResult", return_value=response):
            event = search("needle", kind="album")
        self.assertEqual(event["type"], "playlists")
        self.assertEqual(event["items"][0]["kind"], "album")
        self.assertEqual(event["items"][0]["count"], 12)

    def test_artist_collection_returns_songs_instead_of_falling_through(self):
        with (
            patch(
                "backend.library_api.apis.artist.GetArtistTracks",
                return_value={"songs": [{"id": 9, "name": "track", "dt": 1000}]},
            ),
            patch(
                "backend.library_api.apis.artist.GetArtistDetails",
                return_value={"data": {"name": "artist", "briefDesc": "bio"}},
            ),
        ):
            event = collection("artist", 77)
        self.assertEqual(event["type"], "songs")
        self.assertEqual(event["title"], "artist")
        self.assertEqual(event["items"][0]["id"], 9)

    @patch("backend.library_api._daily_songs")
    def test_daily_recommendations_have_a_visible_title(self, daily_songs):
        daily_songs.return_value = {
            "data": {"dailySongs": [{"id": 10, "name": "daily", "dt": 1000}]}
        }
        event = daily()
        self.assertEqual(event["items"][0]["id"], 10)
        self.assertEqual(event["title"], "每日推荐")

    def test_collection_paging_preserves_search_kind_with_colons(self):
        self.assertEqual(
            parse_collection_value("search:lofi beats:0:100"),
            ("search:lofi beats", 0, 100),
        )
        self.assertEqual(parse_collection_value("playlist:42:200"), ("playlist", 42, 200))

    def test_offline_store_tracks_history_and_survives_missing_file(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "library.json"
            with patch.object(offline, "PATH", path):
                self.assertIsNone(offline.load("missing"))
                offline.store(
                    "search:song:test",
                    {"type": "songs", "items": [{"title": "cached song"}]},
                )
                self.assertEqual(offline.load("search:song:test")["type"], "songs")
                self.assertEqual(offline.history()[-1]["title"], "cached song")
                offline.record_recent(
                    {"id": 1, "title": "recent", "artist": "artist", "duration_ms": 1}
                )
                self.assertEqual(offline.recent()[0]["title"], "recent")
                offline.clear()
                self.assertEqual(offline.history(), [])

    def test_cache_prunes_old_entries_and_orphan_cover(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cover = root / "happybirthday-cover-test.rgb"
            cover.write_bytes(b"x")
            for index in range(3):
                payload = {
                    "cached_at": time.time() + index,
                    "cover": {
                        "path": str(cover) if index == 0 else "",
                        "particle_path": "",
                    },
                }
                path = root / f"{index}.json"
                path.write_text(json.dumps(payload))
                time.sleep(0.01)
            removed = prune_cache(root, max_files=1)
            self.assertEqual(removed, 2)
            self.assertFalse(cover.exists())

    def test_persisted_cache_never_keeps_stream_tokens(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "track.json"
            path.write_text(
                json.dumps(
                    {
                        "track": {
                            "audio_url": "https://example.test/a.mp3?token=secret",
                            "audio_type": "mp3",
                        }
                    }
                )
            )
            self.assertEqual(scrub_persisted_urls(Path(directory)), 1)
            text = path.read_text()
            self.assertNotIn("secret", text)
            self.assertEqual(json.loads(text)["track"]["audio_url"], "")

    def test_network_failure_emits_cached_result(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(offline, "PATH", Path(directory) / "library.json"):
                offline.store("browse:daily:0", {"type": "songs", "items": [{"id": 1}]})
                emitter = Mock()

                def unavailable():
                    raise OSError("injected network failure")

                _cached_emit(emitter, "browse:daily:0", unavailable)
                event = emitter.emit.call_args.args[0]
                self.assertTrue(event["cached"])
                self.assertEqual(event["items"][0]["id"], 1)


if __name__ == "__main__":
    unittest.main()
