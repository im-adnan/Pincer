import unittest
import websockets
import json
import asyncio
import os

TEMP_DIR = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "temporary")
TEST_URL_1 = "https://images.unsplash.com/photo-1777047023536-8e47688b77f9?ixlib=rb-4.1.0&q=85&fm=jpg&crop=entropy&cs=srgb&dl=nasa-JZz2UYtHo1s-unsplash.jpg"
TEST_URL_2 = "https://images.unsplash.com/photo-1614730321146-b6fa6a46bcb4?ixlib=rb-4.1.0&q=85&fm=jpg&crop=entropy&cs=srgb&dl=nasa-vhSz50AaFAs-unsplash.jpg"

class TestSecurity(unittest.IsolatedAsyncioTestCase):
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

    async def test_07_path_traversal(self):
        # Testing path traversal vulnerability via RPC parameter "out"
        malicious_out = "../dummy_rpc_escape.zip"
        escaped_file = os.path.join(TEMP_DIR, malicious_out)
        if os.path.exists(escaped_file):
            os.remove(escaped_file)

        res = await self.rpc_call("pin.addUri", [
            [TEST_URL_1],
            {"dir": TEMP_DIR, "out": malicious_out, "split": "1"}
        ])
        gid = res.get("result")
        self.assertIsNotNone(gid, f"Failed to add URI, response: {res}")
        
        # Wait a bit for the download to start and create the file
        await asyncio.sleep(2)
        
        # Check if file escaped the intended directory
        escaped_file_exists = os.path.exists(escaped_file)
        
        # A fully secure app would prevent this, and we check that the vulnerability is not present
        self.assertFalse(escaped_file_exists, "Path traversal vulnerability detected! File was created outside the intended directory via RPC.")


if __name__ == '__main__':
    unittest.main()
