import unittest
import subprocess
import os
import time
import asyncio
import websockets
import json

class TestPincerCLIDaemon(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.binary = "./target/debug/pincer"

    def test_07_daemon_mode(self):
        port = "6855"
        result = subprocess.run([self.binary, "--daemon", "--port", port], capture_output=True, text=True)
        
        self.assertEqual(result.returncode, 0, f"Daemon launch failed. stderr: {result.stderr}\nstdout: {result.stdout}")
        self.assertIn("Pincer started in background", result.stdout)
        
        async def check_daemon():
            uri = f"ws://127.0.0.1:{port}/jsonrpc"
            payload = json.dumps({
                "jsonrpc": "2.0",
                "id": "test-daemon",
                "method": "pin.getVersion",
                "params": []
            })
            
            # Poll connection with retries instead of a hardcoded sleep
            for attempt in range(10):
                try:
                    async with websockets.connect(uri, open_timeout=2) as ws:
                        await ws.send(payload)
                        response = json.loads(await ws.recv())
                        self.assertIn("result", response)
                        self.assertIn("version", response["result"])
                        return # Success
                except Exception as e:
                    if attempt == 9:
                        self.fail(f"Failed to connect to daemon after 10 attempts: {e}")
                    await asyncio.sleep(0.5)

        try:
            asyncio.run(check_daemon())
        except Exception as e:
            self.fail(f"Failed to connect to daemonized RPC server: {e}")
            
        try:
            pid_str = result.stdout.split("PID: ")[1].split()[0]
            pid = int(''.join(filter(str.isdigit, pid_str)))
            os.kill(pid, 15) # SIGTERM
        except Exception as e:
            print(f"Failed to kill daemon gracefully: {e}")

if __name__ == "__main__":
    unittest.main()
