import unittest
import websockets
import json
import asyncio
import os

TEMP_DIR = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "temporary")
TEST_URL_1 = "https://images.unsplash.com/photo-1777047023536-8e47688b77f9?ixlib=rb-4.1.0&q=85&fm=jpg&crop=entropy&cs=srgb&dl=nasa-JZz2UYtHo1s-unsplash.jpg"
TEST_URL_2 = "https://images.unsplash.com/photo-1614730321146-b6fa6a46bcb4?ixlib=rb-4.1.0&q=85&fm=jpg&crop=entropy&cs=srgb&dl=nasa-vhSz50AaFAs-unsplash.jpg"

class TestGlobalState(unittest.IsolatedAsyncioTestCase):
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

    async def test_01_get_version(self):
        res = await self.rpc_call("pin.getVersion")
        self.assertIn("result", res)
        self.assertIn("version", res["result"])

    async def test_02_global_options(self):
        # getGlobalOption
        res = await self.rpc_call("pin.getGlobalOption")
        self.assertIn("result", res, f"Error getting global option: {res}")
        self.assertIsInstance(res["result"], dict)

        # changeGlobalOption
        res2 = await self.rpc_call("pin.changeGlobalOption", [{"max-concurrent-downloads": "5"}])
        self.assertEqual(res2.get("result"), "OK", f"Error changing global option: {res2}")

    async def test_04_bulk_and_stats(self):
        res = await self.rpc_call("pin.pauseAll")
        self.assertEqual(res.get("result"), "OK", f"Error pauseAll: {res}")

        res = await self.rpc_call("pin.unpauseAll")
        self.assertEqual(res.get("result"), "OK", f"Error unpauseAll: {res}")

        res = await self.rpc_call("pin.tellActive")
        self.assertIsInstance(res.get("result"), list, f"Error tellActive: {res}")

        res = await self.rpc_call("pin.tellWaiting")
        self.assertIsInstance(res.get("result"), list, f"Error tellWaiting: {res}")

        res = await self.rpc_call("pin.tellStopped")
        self.assertIsInstance(res.get("result"), list, f"Error tellStopped: {res}")

        res = await self.rpc_call("pin.getGlobalStat")
        self.assertIsInstance(res.get("result"), dict, f"Error getGlobalStat: {res}")

    async def test_05_cleanup(self):
        res = await self.rpc_call("pin.purgeDownloadResult")
        self.assertEqual(res.get("result"), "OK", f"Error purgeDownloadResult: {res}")

        add_res = await self.rpc_call("pin.addUri", [
            [TEST_URL_2],
            {"dir": TEMP_DIR, "out": "dummy.zip"}
        ])
        gid = add_res.get("result")
        self.assertIsNotNone(gid, f"Failed to add dummy URI, response: {add_res}")
        
        # forceRemove
        res = await self.rpc_call("pin.forceRemove", [gid])
        self.assertNotIn("error", res, f"Error forceRemove: {res}")

        # removeDownloadResult (might be gone already, just make sure RPC handles it without crashing server)
        res = await self.rpc_call("pin.removeDownloadResult", [gid])
        # Either it's OK or error because it's not found, both are acceptable RPC behaviors
        self.assertIn("jsonrpc", res)

    async def test_06_misc(self):
        res = await self.rpc_call("pin.saveSession")
        self.assertEqual(res.get("result"), "OK", f"Error saveSession: {res}")

        # Testing resolveUrl
        res = await self.rpc_call("pin.resolveUrl", ["https://github.com"])
        self.assertNotIn("error", res, f"Error resolveUrl: {res}")

    async def test_10_system_methods(self):
        # system.listMethods
        res = await self.rpc_call("system.listMethods")
        self.assertIn("result", res, f"Error listMethods: {res}")
        self.assertIsInstance(res["result"], list)
        self.assertIn("pin.addUri", res["result"])

        # system.listNotifications
        res = await self.rpc_call("system.listNotifications")
        self.assertIn("result", res, f"Error listNotifications: {res}")
        self.assertIsInstance(res["result"], list)
        self.assertIn("pin.onDownloadStart", res["result"])

        # system.multicall
        res = await self.rpc_call("system.multicall", [[
            {"methodName": "pin.getVersion", "params": []},
            {"methodName": "system.listNotifications", "params": []}
        ]])
        self.assertIn("result", res, f"Error multicall: {res}")
        self.assertIsInstance(res["result"], list)
        self.assertEqual(len(res["result"]), 2)
        self.assertIn("version", res["result"][0][0])
        self.assertIn("pin.onDownloadStart", res["result"][1][0])


if __name__ == '__main__':
    unittest.main()
