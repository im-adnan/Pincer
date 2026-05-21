import unittest
import subprocess
import os

class TestPincerCLI(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # Build the binary once for all CLI tests
        print("Building Pincer binary for CLI tests...")
        subprocess.run(["cargo", "build"], check=True)
        cls.binary = "./target/debug/pincer"

    def test_01_version(self):
        result = subprocess.run([self.binary, "--version"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0)
        self.assertIn("pincer", result.stdout)

    def test_02_help(self):
        result = subprocess.run([self.binary, "--help"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0)
        self.assertIn("Usage: pincer [URL] [OPTIONS]", result.stdout)

    def test_03_download_file(self):
        test_out = "dummy_cli_test.zip"
        if os.path.exists(test_out):
            os.remove(test_out)
            
        result = subprocess.run([
            self.binary, 
            "https://example.com/dummy.zip",
            "--out", test_out,
            "--split", "2"
        ], capture_output=True, text=True)
        
        self.assertEqual(result.returncode, 0, f"Download failed. stderr: {result.stderr}\nstdout: {result.stdout}")
        self.assertTrue(os.path.exists(test_out), "Output file was not created.")
        
        # Cleanup
        if os.path.exists(test_out):
            os.remove(test_out)

if __name__ == "__main__":
    unittest.main()
