import unittest
import websockets
import json
import asyncio
import os

TEMP_DIR = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "temporary")
TEST_URL_1 = "https://images.unsplash.com/photo-1777047023536-8e47688b77f9?ixlib=rb-4.1.0&q=85&fm=jpg&crop=entropy&cs=srgb&dl=nasa-JZz2UYtHo1s-unsplash.jpg"
TEST_URL_2 = "https://images.unsplash.com/photo-1614730321146-b6fa6a46bcb4?ixlib=rb-4.1.0&q=85&fm=jpg&crop=entropy&cs=srgb&dl=nasa-vhSz50AaFAs-unsplash.jpg"

class TestAdvanced(unittest.IsolatedAsyncioTestCase):
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

    async def test_11_task_introspection(self):
        # Start a dummy task
        res = await self.rpc_call("pin.addUri", [
            [TEST_URL_1],
            {"dir": TEMP_DIR, "out": "dummy_introspection.zip"}
        ])
        gid = res.get("result")
        self.assertIsNotNone(gid, f"Failed to add URI, response: {res}")

        # getSessionInfo
        res = await self.rpc_call("pin.getSessionInfo")
        self.assertIn("result", res)
        self.assertIn("sessionId", res["result"])

        # getFiles
        res = await self.rpc_call("pin.getFiles", [gid])
        self.assertIn("result", res)
        self.assertIsInstance(res["result"], list)
        self.assertTrue(len(res["result"]) > 0)
        self.assertIn("path", res["result"][0])

        # getUris
        res = await self.rpc_call("pin.getUris", [gid])
        self.assertIn("result", res)
        self.assertIsInstance(res["result"], list)
        self.assertTrue(len(res["result"]) > 0)
        self.assertIn("uri", res["result"][0])

        # getServers
        res = await self.rpc_call("pin.getServers", [gid])
        self.assertIn("result", res)
        self.assertIsInstance(res["result"], list)
        self.assertTrue(len(res["result"]) > 0)
        self.assertIn("servers", res["result"][0])

        # Cleanup
        await self.rpc_call("pin.forceRemove", [gid])

    async def test_12_advanced_task_modification(self):
        # Start a dummy task
        res = await self.rpc_call("pin.addUri", [
            [TEST_URL_1],
            {"dir": TEMP_DIR, "out": "dummy_mod.zip"}
        ])
        gid = res.get("result")
        
        # changePosition
        res = await self.rpc_call("pin.changePosition", [gid, 0, "POS_SET"])
        self.assertIn("result", res)
        self.assertEqual(res["result"], 0)

        # changeUri (must pause first)
        await self.rpc_call("pin.pause", [gid])
        
        # wait a bit for pause
        await asyncio.sleep(1)
        
        res = await self.rpc_call("pin.changeUri", [gid, 0, [TEST_URL_1], ["http://example.com/new"]])
        self.assertIn("result", res, f"changeUri failed: {res}")
        self.assertEqual(res["result"], [1, 1])

        # Verify new URI
        res = await self.rpc_call("pin.getUris", [gid])
        self.assertEqual(res["result"][0]["uri"], "http://example.com/new")

        # Cleanup
        await self.rpc_call("pin.forceRemove", [gid])

    async def test_13_parameterized_uris(self):
        # Add single parameterized URI that expands into 3
        res = await self.rpc_call("pin.addUri", [
            ["http://example.com/file[01-03].txt"],
            {"dir": TEMP_DIR, "out": "dummy_param.txt"}
        ])
        
        # Should return an array of 3 GIDs
        self.assertIn("result", res, f"addUri failed: {res}")
        self.assertIsInstance(res["result"], list)
        self.assertEqual(len(res["result"]), 3)
        
        gids = res["result"]
        
        # Verify first task
        res1 = await self.rpc_call("pin.getUris", [gids[0]])
        self.assertEqual(res1["result"][0]["uri"], "http://example.com/file01.txt")
        
        # Verify third task
        res3 = await self.rpc_call("pin.getUris", [gids[2]])
        self.assertEqual(res3["result"][0]["uri"], "http://example.com/file03.txt")
        
        # Add brace parameterized URI
        res = await self.rpc_call("pin.addUri", [
            ["http://{server1,server2}/file.txt"],
            {"dir": TEMP_DIR, "out": "dummy_param2.txt"}
        ])
        
        self.assertIn("result", res)
        self.assertIsInstance(res["result"], list)
        self.assertEqual(len(res["result"]), 2)
        
        # Cleanup
        for gid in gids + res["result"]:
            await self.rpc_call("pin.forceRemove", [gid])

if __name__ == '__main__':
    unittest.main()
