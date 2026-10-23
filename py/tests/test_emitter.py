import io
import json
import unittest

from backend.emitter import JsonEmitter


class EmitterTests(unittest.TestCase):
    def test_events_are_serialized_in_order(self):
        stream = io.StringIO()
        emitter = JsonEmitter(stream)
        emitter.emit({"type": "meta", "title": "曲名"})
        emitter.emit({"type": "state", "state": "ended"})
        emitter.close()
        rows = [json.loads(line) for line in stream.getvalue().splitlines()]
        self.assertEqual([row["type"] for row in rows], ["meta", "state"])
        self.assertEqual(rows[0]["title"], "曲名")


if __name__ == "__main__":
    unittest.main()
