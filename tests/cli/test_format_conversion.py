import unittest
import subprocess
import os

TEST_URL = "https://images.unsplash.com/photo-1446941303752-a64bb1048d54?ixlib=rb-4.1.0&q=85&fm=jpg&crop=entropy&cs=srgb&dl=nasa-U2uKrI4lci8-unsplash.jpg"

class TestPincerCLIFormatConversion(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.binary = "./target/debug/pincer"
        cls.temp_dir = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "temporary")
        os.makedirs(cls.temp_dir, exist_ok=True)

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

if __name__ == "__main__":
    unittest.main()
