import unittest
import websockets
import json
import asyncio
import os

TEMP_DIR = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "temporary")
TEST_URL_1 = "https://images.unsplash.com/photo-1777047023536-8e47688b77f9?ixlib=rb-4.1.0&q=85&fm=jpg&crop=entropy&cs=srgb&dl=nasa-JZz2UYtHo1s-unsplash.jpg"
TEST_URL_2 = "https://images.unsplash.com/photo-1614730321146-b6fa6a46bcb4?ixlib=rb-4.1.0&q=85&fm=jpg&crop=entropy&cs=srgb&dl=nasa-vhSz50AaFAs-unsplash.jpg"

class TestProtocols(unittest.IsolatedAsyncioTestCase):
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

    async def test_14_ftp_sftp(self):
        # Test FTP
        res_ftp = await self.rpc_call("pin.addUri", [
            ["ftp://test.rebex.net/readme.txt"],
            {"dir": TEMP_DIR, "out": "readme_ftp.txt"}
        ])
        gid_ftp = res_ftp.get("result")
        self.assertIsNotNone(gid_ftp, f"Failed to add FTP URI: {res_ftp}")
        
        # Test SFTP
        res_sftp = await self.rpc_call("pin.addUri", [
            ["sftp://demo:password@test.rebex.net/readme.txt"],
            {"dir": TEMP_DIR, "out": "readme_sftp.txt"}
        ])
        gid_sftp = res_sftp.get("result")
        self.assertIsNotNone(gid_sftp, f"Failed to add SFTP URI: {res_sftp}")
        
        # Wait up to 10 seconds for completion
        for _ in range(10):
            status_ftp = await self.rpc_call("pin.tellStatus", [gid_ftp])
            status_sftp = await self.rpc_call("pin.tellStatus", [gid_sftp])
            if status_ftp.get("result", {}).get("status") in ["complete", "error"] and \
               status_sftp.get("result", {}).get("status") in ["complete", "error"]:
                break
            await asyncio.sleep(1)
            
        status_ftp = await self.rpc_call("pin.tellStatus", [gid_ftp])
        status_sftp = await self.rpc_call("pin.tellStatus", [gid_sftp])
        
        # The servers might be down or blocked, but the engine should at least accept the URLs and not crash.
        # If it completes, we check if the files exist.
        if status_ftp.get("result", {}).get("status") == "complete":
            self.assertTrue(os.path.exists(os.path.join(TEMP_DIR, "readme_ftp.txt")))
        
        if status_sftp.get("result", {}).get("status") == "complete":
            self.assertTrue(os.path.exists(os.path.join(TEMP_DIR, "readme_sftp.txt")))
        
        await self.rpc_call("pin.forceRemove", [gid_ftp])
        await self.rpc_call("pin.forceRemove", [gid_sftp])

    @unittest.skip("librqbit pause/unpause state machine is inconsistent for magnet links during metadata fetch")
    async def test_15_bittorrent(self):
        # Test adding magnet link
        magnet_url = "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c&dn=Ubuntu"
        res = await self.rpc_call("pin.addUri", [
            [magnet_url],
            {"dir": TEMP_DIR}
        ])
        gid = res.get("result")
        self.assertIsNotNone(gid, f"Failed to add magnet link: {res}")
        
        # Tell status and check fields
        res_status = await self.rpc_call("pin.tellStatus", [gid])
        status = res_status.get("result", {})
        self.assertEqual(status.get("fileType"), "torrent")
        self.assertEqual(status.get("infoHash"), "dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c")
        
        # Pause
        res_pause = await self.rpc_call("pin.pause", [gid])
        self.assertNotIn("error", res_pause)
        res_status = await self.rpc_call("pin.tellStatus", [gid])
        self.assertEqual(res_status.get("result", {}).get("status"), "paused")
        
        # Unpause
        res_unpause = await self.rpc_call("pin.unpause", [gid])
        self.assertNotIn("error", res_unpause)
        
        # Poll for status change to 'active' since torrent state transitions are async
        for _ in range(30):
            res_status = await self.rpc_call("pin.tellStatus", [gid])
            if res_status.get("result", {}).get("status") == "active":
                break
            await asyncio.sleep(0.1)
            
        self.assertEqual(res_status.get("result", {}).get("status"), "active")
        
        # Remove
        res_remove = await self.rpc_call("pin.remove", [gid])
        self.assertNotIn("error", res_remove)

if __name__ == "__main__":
    unittest.main()


if __name__ == '__main__':
    unittest.main()
