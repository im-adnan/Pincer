import unittest
import websockets
import json
import asyncio
import os

TEMP_DIR = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "temporary")
TEST_URL_1 = "https://images.unsplash.com/photo-1777047023536-8e47688b77f9?ixlib=rb-4.1.0&q=85&fm=jpg&crop=entropy&cs=srgb&dl=nasa-JZz2UYtHo1s-unsplash.jpg"
TEST_URL_2 = "https://images.unsplash.com/photo-1614730321146-b6fa6a46bcb4?ixlib=rb-4.1.0&q=85&fm=jpg&crop=entropy&cs=srgb&dl=nasa-vhSz50AaFAs-unsplash.jpg"

class TestAuth(unittest.IsolatedAsyncioTestCase):
    URI = "ws://127.0.0.1:6842/jsonrpc"

    @classmethod
    def setUpClass(cls):
        os.makedirs(TEMP_DIR, exist_ok=True)

    async def rpc_call(self, method, params=None):
        req_id = f"test-{method}"
        if params is None:
            params = []
        payload = {
            "jsonrpc": "2.0",
            "id": req_id,
            "method": method,
            "params": params
        }
        try:
            async with websockets.connect(self.URI, open_timeout=5) as ws:
                await ws.send(json.dumps(payload))
                while True:
                    response = json.loads(await ws.recv())
                    if response.get("id") == req_id:
                        return response
        except Exception as e:
            self.fail(f"Failed to connect or communicate with RPC server: {e}")

    async def test_09_auth(self):
        # Start a server with a secret on a different port
        auth_port = 6843
        secret = "my_super_secret"
        import subprocess, time
        server_proc = subprocess.Popen(
            ["./target/debug/pincer", "--port", str(auth_port), "--rpc-secret", secret],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL
        )
        time.sleep(2) # Give it time to bind

        uri = f"ws://127.0.0.1:{auth_port}/jsonrpc"
        
        # Test 1: No secret -> Unauthorized
        payload_no_secret = {
            "jsonrpc": "2.0",
            "id": "test-auth-fail",
            "method": "pin.getVersion",
            "params": []
        }
        try:
            async with websockets.connect(uri, open_timeout=5) as ws:
                await ws.send(json.dumps(payload_no_secret))
                response = json.loads(await ws.recv())
                self.assertIn("error", response)
                self.assertEqual(response["error"]["message"], "Unauthorized")
        except Exception as e:
            self.fail(f"Failed to connect: {e}")

        # Test 2: Valid secret -> Success
        payload_valid = {
            "jsonrpc": "2.0",
            "id": "test-auth-success",
            "method": "pin.getVersion",
            "params": [f"token:{secret}"]
        }
        try:
            async with websockets.connect(uri, open_timeout=5) as ws:
                await ws.send(json.dumps(payload_valid))
                response = json.loads(await ws.recv())
                self.assertNotIn("error", response)
                self.assertIn("result", response)
        except Exception as e:
            self.fail(f"Failed to connect: {e}")

        # Cleanup
        server_proc.terminate()
        server_proc.wait(timeout=5)


if __name__ == '__main__':
    unittest.main()
