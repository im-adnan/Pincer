import unittest
import subprocess
import os

TEST_URL = "https://images.unsplash.com/photo-1446941303752-a64bb1048d54?ixlib=rb-4.1.0&q=85&fm=jpg&crop=entropy&cs=srgb&dl=nasa-U2uKrI4lci8-unsplash.jpg"

class TestPincerCLI(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        # The binary is already built by run_tests.py
        cls.binary = "./target/debug/pincer"
        
        # Setup temporary folder
        cls.temp_dir = os.path.join(os.path.dirname(os.path.abspath(__file__)), "temporary")
        os.makedirs(cls.temp_dir, exist_ok=True)

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
        dest_path = os.path.join(self.temp_dir, test_out)
        if os.path.exists(dest_path):
            os.remove(dest_path)
            
        result = subprocess.run([
            self.binary, 
            TEST_URL,
            "--out", test_out,
            "--dir", self.temp_dir,
            "--split", "2"
        ], capture_output=True, text=True)
        
        self.assertEqual(result.returncode, 0, f"Download failed. stderr: {result.stderr}\nstdout: {result.stdout}")
        self.assertTrue(os.path.exists(dest_path), "Output file was not created.")

    def test_04_format_conversion(self):
        test_out = "dummy_format.jpg"
        expected_out = "dummy_format.png"
        dest_in = os.path.join(self.temp_dir, test_out)
        dest_out = os.path.join(self.temp_dir, expected_out)
        
        if os.path.exists(dest_in):
            os.remove(dest_in)
        if os.path.exists(dest_out):
            os.remove(dest_out)
            
        result = subprocess.run([
            self.binary, 
            TEST_URL,
            "--out", test_out,
            "--dir", self.temp_dir,
            "--format", "png"
        ], capture_output=True, text=True)
        
        self.assertEqual(result.returncode, 0, f"Download with format failed. stderr: {result.stderr}\nstdout: {result.stdout}")

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
        ], capture_output=True, text=True)
        
        self.assertEqual(result.returncode, 0)
        
        # If the vulnerability exists, the file is created at tests/dummy_escape.zip
        escaped_file_exists = os.path.exists(escaped_file)
        
        # A fully secure app would prevent this, and we check that the vulnerability is not present
        self.assertFalse(escaped_file_exists, "Path traversal vulnerability detected! File was created outside the intended directory.")

if __name__ == "__main__":
    unittest.main()


