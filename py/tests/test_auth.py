import unittest
from unittest.mock import patch

from backend.auth import qr_lines, require_login, user_playlist_rows


class AuthTests(unittest.TestCase):
    def test_qr_uses_compact_braille_matrix_that_fits_small_terminals(self):
        lines = qr_lines("https://music.163.com/login?codekey=test")
        self.assertGreater(len(lines), 8)
        self.assertLessEqual(len(lines), 14)
        widths = {len(line) for line in lines}
        self.assertEqual(len(widths), 1)
        self.assertTrue(
            all(0x2800 <= ord(char) <= 0x28FF for char in "".join(lines))
        )
        self.assertTrue(any(char != "\u2800" for char in "".join(lines)))

    @patch("backend.auth.clear_session")
    @patch("backend.auth.user_profile", return_value={"uid": 0})
    def test_expired_login_is_detected_and_cleared(self, profile, clear_session):
        with self.assertRaisesRegex(RuntimeError, "auth_expired|登录"):
            require_login()
        clear_session.assert_called_once()

    @patch("backend.auth.apis.user.GetUserPlaylists")
    def test_playlist_rows_are_paginated_and_deduplicated(self, get_playlists):
        first = [{"id": index} for index in range(1, 501)]
        second = [{"id": 500}, {"id": 501}]
        get_playlists.side_effect = [{"playlist": first}, {"playlist": second}]
        rows = user_playlist_rows(123)
        self.assertEqual(len(rows), 501)
        self.assertEqual(get_playlists.call_args_list[1].kwargs["offset"], 500)


if __name__ == "__main__":
    unittest.main()
