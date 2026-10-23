import os
import socket
import tempfile
import threading
import unittest
from array import array

from backend.pcm import PcmSender


class PcmSenderTests(unittest.TestCase):
    @unittest.skipUnless(hasattr(socket, "AF_UNIX"), "Unix sockets are unavailable")
    def test_stream_protocol(self):
        path = os.path.join(
            tempfile.gettempdir(), f"epsilonanime-test-{os.getpid()}.sock"
        )
        try:
            os.unlink(path)
        except FileNotFoundError:
            pass
        server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        server.bind(path)
        server.listen(1)
        capture = {}

        def receive():
            connection, _ = server.accept()
            stream = connection.makefile("rb")
            header = stream.read(12)
            packet = stream.read(8)
            size = int.from_bytes(packet[4:8], "little")
            payload = stream.read(size)
            capture["header"] = header
            capture["packet"] = packet + payload
            connection.close()

        thread = threading.Thread(target=receive)
        thread.start()
        sender = PcmSender(path, 44100)
        samples = array("f", [0.1, 0.2, 0.3, 0.4])
        sender.send(samples, 1.25)
        sender.close()
        thread.join(timeout=2)
        server.close()
        os.unlink(path)

        self.assertEqual(capture["header"][:4], b"EAP1")
        self.assertEqual(int.from_bytes(capture["header"][4:8], "little"), 44100)
        self.assertEqual(len(capture["packet"]), 8 + len(samples) * 4)


if __name__ == "__main__":
    unittest.main()
