import unittest
import subprocess
import os

TEST_URL = "https://raw.githubusercontent.com/rust-lang/cargo/master/README.md"

class TestPincerCLIDownload(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.binary = "./target/debug/pincer"
        cls.temp_dir = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "temporary")
        os.makedirs(cls.temp_dir, exist_ok=True)

    def test_03_download_file(self):
        test_out = "dummy_cli_test.zip"
        dest_path = os.path.join(self.temp_dir, test_out)
        if os.path.exists(dest_path):
            os.remove(dest_path)
            
        result = subprocess.run([
            self.binary, 
            TEST_URL,
            "--out", test_out,
            "--dir", self.temp_dir,
            "--split", "2"
        ], capture_output=True, text=True, timeout=15)
        
        self.assertEqual(result.returncode, 0, f"Download failed. stderr: {result.stderr}\nstdout: {result.stdout}")
        self.assertTrue(os.path.exists(dest_path), "Output file was not created.")

    def test_05_path_traversal(self):
        # Testing path traversal vulnerability in --out parameter
        malicious_out = "../dummy_escape.zip"
        escaped_file = os.path.join(self.temp_dir, malicious_out)
        if os.path.exists(escaped_file):
            os.remove(escaped_file)
            
        result = subprocess.run([
            self.binary, 
            TEST_URL,
            "--out", malicious_out,
            "--dir", self.temp_dir
        ], capture_output=True, text=True, timeout=15)
        
        self.assertEqual(result.returncode, 0)
        
        # If the vulnerability exists, the file is created at tests/dummy_escape.zip
        escaped_file_exists = os.path.exists(escaped_file)
        
        # A fully secure app would prevent this, and we check that the vulnerability is not present
        self.assertFalse(escaped_file_exists, "Path traversal vulnerability detected! File was created outside the intended directory.")

if __name__ == "__main__":
    unittest.main()
