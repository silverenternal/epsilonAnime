import io
import unittest

from backend.control import ControlBus


class ControlBusTests(unittest.TestCase):
    def test_parses_json_commands_and_skips_malformed_lines(self):
        stream = io.StringIO('{"cmd":"pause"}\nnot json\n{"cmd":"resume"}\n')
        bus = ControlBus(stream)
        bus.start()
        bus.thread.join(timeout=1)
        self.assertEqual(bus.drain(), [{"cmd": "pause"}, {"cmd": "resume"}])
        self.assertTrue(bus.closed.is_set())

    def test_preload_waits_for_start_and_preserves_other_commands(self):
        stream = io.StringIO('{"cmd":"volume","value":0.5}\n{"cmd":"start"}\n')
        bus = ControlBus(stream)
        bus.start()
        self.assertEqual(bus.wait_for("start", timeout=1), {"cmd": "start"})
        self.assertEqual(bus.drain(), [{"cmd": "volume", "value": 0.5}])


if __name__ == "__main__":
    unittest.main()
