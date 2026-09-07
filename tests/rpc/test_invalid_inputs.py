import unittest
import websockets
import json
import asyncio
import os

TEMP_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "temporary")

class TestInvalidInputs(unittest.IsolatedAsyncioTestCase):
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

    async def test_01_missing_parameters(self):
        # Missing required parameters for addUri
        res = await self.rpc_call("pin.addUri", [])
        self.assertIn("error", res, "Should return error for missing URIs")
        
    async def test_02_invalid_json(self):
        # Send raw invalid JSON string
        try:
            async with websockets.connect(self.URI, open_timeout=5) as ws:
                await ws.send("{invalid json")
                try:
                    res = await asyncio.wait_for(ws.recv(), timeout=2.0)
                    response = json.loads(res)
                    self.assertIn("error", response, "Should return parse error")
                except (asyncio.TimeoutError, websockets.exceptions.ConnectionClosed):
                    pass # Handled safely if it drops or ignores
        except Exception as e:
            self.fail(f"Failed to communicate with RPC server: {e}")

    async def test_03_extreme_split_argument(self):
        # Test providing an extremely large split value
        res = await self.rpc_call("pin.addUri", [
            ["http://example.com/dummy.txt"],
            {"dir": TEMP_DIR, "split": "9999"}
        ])
        
        # Depending on engine behavior, this might succeed but cap the split internally, or reject it.
        # Assuming it succeeds, we check that it didn't panic and returned a valid GID.
        self.assertIn("result", res, f"Failed to handle extreme split value: {res}")
        gid = res["result"]
        
        # Clean up
        await self.rpc_call("pin.remove", [gid])

if __name__ == '__main__':
    unittest.main()
