import unittest
import websockets
import json
import base64
import asyncio
import os
import time

TEMP_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "temporary")

class TestMetalink(unittest.IsolatedAsyncioTestCase):
    URI = "ws://127.0.0.1:6842/jsonrpc"

    @classmethod
    def setUpClass(cls):
        os.makedirs(TEMP_DIR, exist_ok=True)

    async def rpc_call(self, method, params=None):
        if params is None:
            params = []
        payload = {
            "jsonrpc": "2.0",
            "id": f"test-{method}",
            "method": method,
            "params": params
        }
        try:
            async with websockets.connect(self.URI, open_timeout=5) as ws:
                await ws.send(json.dumps(payload))
                response = await ws.recv()
                return json.loads(response)
        except Exception as e:
            self.fail(f"Failed to connect or communicate with RPC server: {e}")

    async def test_01_metalink_parsing_and_fallback(self):
        real_url = "https://images.unsplash.com/photo-1446941303752-a64bb1048d54?ixlib=rb-4.1.0&q=85&fm=jpg&crop=entropy&cs=srgb&dl=nasa-U2uKrI4lci8-unsplash.jpg".replace("&", "&amp;")
        dead_url = "http://127.0.0.1:9999/doesnotexist.jpg".replace("&", "&amp;")

        xml = f"""<?xml version="1.0" encoding="utf-8"?>
<metalink xmlns="urn:ietf:params:xml:ns:metalink">
  <file name="nasa_test.jpg">
    <url priority="1">{dead_url}</url>
    <url priority="2">{real_url}</url>
  </file>
</metalink>"""

        b64 = base64.b64encode(xml.encode("utf-8")).decode("utf-8")
        
        res = await self.rpc_call("pin.addMetalink", [b64, {"dir": TEMP_DIR, "split": "2"}])
        self.assertIn("result", res, f"Expected result in {res}")
        
        gid = res["result"]
        if isinstance(gid, list):
            gid = gid[0]

        status = "active"
        while status in ["active", "waiting"]:
            await asyncio.sleep(1)
            tell_res = await self.rpc_call("pin.tellStatus", [gid])
            status = tell_res["result"]["status"]

        self.assertEqual(status, "complete", f"Task did not complete successfully. Status: {status}")
        
        dest_path = tell_res["result"]["files"][0]["path"]
        self.assertTrue(os.path.exists(dest_path), "File was not downloaded")

    async def test_02_metalink_checksum_failure(self):
        real_url = "https://images.unsplash.com/photo-1446941303752-a64bb1048d54?ixlib=rb-4.1.0&q=85&fm=jpg&crop=entropy&cs=srgb&dl=nasa-U2uKrI4lci8-unsplash.jpg".replace("&", "&amp;")

        xml = f"""<?xml version="1.0" encoding="utf-8"?>
<metalink xmlns="urn:ietf:params:xml:ns:metalink">
  <file name="nasa_test_hash.jpg">
    <hash type="sha-256">0000000000000000000000000000000000000000000000000000000000000000</hash>
    <url priority="1">{real_url}</url>
  </file>
</metalink>"""

        b64 = base64.b64encode(xml.encode("utf-8")).decode("utf-8")
        
        res = await self.rpc_call("pin.addMetalink", [b64, {"dir": TEMP_DIR}])
        self.assertIn("result", res, f"Expected result in {res}")
        
        gid = res["result"]
        if isinstance(gid, list):
            gid = gid[0]

        status = "active"
        while status in ["active", "waiting"]:
            await asyncio.sleep(1)
            tell_res = await self.rpc_call("pin.tellStatus", [gid])
            status = tell_res["result"]["status"]

        self.assertEqual(status, "error", f"Task should have failed checksum validation. Status: {status}")

if __name__ == "__main__":
    unittest.main()
