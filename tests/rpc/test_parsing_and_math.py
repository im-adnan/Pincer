import unittest
import websockets
import json
import asyncio
import os
import base64

class TestParsingAndMath(unittest.IsolatedAsyncioTestCase):
    URI = "ws://127.0.0.1:6842/jsonrpc"

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

    async def test_01_malformed_metalink(self):
        # Base64 encode a malformed XML string
        malformed_xml = "<metalink><file name=\"unclosed\">"
        b64_xml = base64.b64encode(malformed_xml.encode('utf-8')).decode('utf-8')
        
        res = await self.rpc_call("pin.addMetalink", [b64_xml])
        self.assertIn("error", res, "Should return error for malformed metalink XML")
        self.assertNotIn("result", res)

if __name__ == '__main__':
    unittest.main()
