import unittest
import subprocess
import os

class TestPincerCLIBasic(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.binary = "./target/debug/pincer"

    def test_01_version(self):
        result = subprocess.run([self.binary, "--version"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0)
        self.assertIn("pincer", result.stdout)

    def test_02_help(self):
        result = subprocess.run([self.binary, "--help"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0)
        self.assertIn("Usage: pincer [OPTIONS] [URL | .torrent | .metalink | magnet:?]", result.stdout)

    def test_06_port_help(self):
        # Verify that --port option is accepted by parser when combined with help
        result = subprocess.run([self.binary, "--help", "--port", "12345"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0)
        self.assertIn("RPC WebSocket listener port", result.stdout)

if __name__ == "__main__":
    unittest.main()
