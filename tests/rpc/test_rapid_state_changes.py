import unittest
import websockets
import json
import asyncio
import os

TEMP_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "temporary")

class TestRapidStateChanges(unittest.IsolatedAsyncioTestCase):
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

    async def test_01_rapid_pause_unpause(self):
        # Add a dummy task that takes some time
        res = await self.rpc_call("pin.addUri", [
            ["http://speedtest.ftp.otenet.gr/files/test1Gb.db"],
            {"dir": TEMP_DIR, "out": "dummy_speedtest.db", "split": "4"}
        ])
        self.assertIn("result", res, f"Failed to add task: {res}")
        gid = res["result"]
        
        # Rapidly pause and unpause
        for _ in range(5):
            pause_res = await self.rpc_call("pin.pause", [gid])
            self.assertIn("result", pause_res)
            
            unpause_res = await self.rpc_call("pin.unpause", [gid])
            self.assertIn("result", unpause_res)
            
            await asyncio.sleep(0.1)
            
        # Clean up
        await self.rpc_call("pin.remove", [gid])
        
    async def test_02_mass_pause(self):
        # Spawn multiple tasks
        gids = []
        for i in range(5):
            res = await self.rpc_call("pin.addUri", [
                [f"http://speedtest.ftp.otenet.gr/files/test1Gb.db?dummy={i}"],
                {"dir": TEMP_DIR, "out": f"dummy_mass_{i}.db"}
            ])
            self.assertIn("result", res)
            gids.append(res["result"])
            
        # Pause all
        pause_all_res = await self.rpc_call("pin.pauseAll")
        self.assertIn("result", pause_all_res)
        self.assertEqual(pause_all_res["result"], "OK")
        
        # Verify status is paused
        for gid in gids:
            status_res = await self.rpc_call("pin.tellStatus", [gid])
            self.assertEqual(status_res["result"]["status"], "paused")
            
        # Clean up
        for gid in gids:
            await self.rpc_call("pin.remove", [gid])

if __name__ == '__main__':
    unittest.main()
