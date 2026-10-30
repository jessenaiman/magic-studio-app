"""Studio window identity: pid from the socket name, and preview servers matched to open windows.

Run: PYTHONPATH=agent-harness python -m unittest discover -s agent-harness/tests -v
"""

import unittest
from unittest import mock

from cli_anything.magic.core.session import Session
from cli_anything.magic.utils import magic_backend as backend


class StudioIdentityTests(unittest.TestCase):
    def test_pid_comes_from_socket_name(self):
        self.assertEqual(backend.studio_pid("3460392-1793379659546059562.sock"), 3460392)
        self.assertIsNone(backend.studio_pid("odd.sock"))

    def test_unavailable_pin_names_the_live_window_without_rebinding(self):
        session = Session.__new__(Session)
        session.project_path = "/p/Song.magic"
        live = {"sessions": [{"pid": 7, "instance": "7-1.sock", "canonical_project": "/p/Song.magic"}], "errors": []}
        with mock.patch.object(backend, "discover_studios", return_value=live):
            message = session._unavailable_message()
        self.assertIn("pid 7", message)
        self.assertIn("studio open /p/Song.magic", message)
        with mock.patch.object(backend, "discover_studios", return_value={"sessions": [], "errors": []}):
            self.assertIn("No live window", session._unavailable_message())


if __name__ == "__main__":
    unittest.main()
