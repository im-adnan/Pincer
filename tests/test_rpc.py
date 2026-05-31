import unittest
import websockets
import json
import asyncio
import os

TEST_URL_1 = "https://images.unsplash.com/photo-1777047023536-8e47688b77f9?ixlib=rb-4.1.0&q=85&fm=jpg&crop=entropy&cs=srgb&dl=nasa-JZz2UYtHo1s-unsplash.jpg"
TEST_URL_2 = "https://images.unsplash.com/photo-1614730321146-b6fa6a46bcb4?ixlib=rb-4.1.0&q=85&fm=jpg&crop=entropy&cs=srgb&dl=nasa-vhSz50AaFAs-unsplash.jpg"

TEMP_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "temporary")

class TestPincerRPC(unittest.IsolatedAsyncioTestCase):
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

    async def test_03_lifecycle(self):
        # addUri
        res = await self.rpc_call("pin.addUri", [
            [TEST_URL_1],
            {"dir": TEMP_DIR, "out": "dummy_test.zip", "split": "1"}
        ])
        gid = res.get("result")
        self.assertIsNotNone(gid, f"Failed to add URI, response: {res}")

        # tellStatus
        res = await self.rpc_call("pin.tellStatus", [gid])
        status = res.get("result", {}).get("status")
        # In a real async environment, it could be active, waiting, complete, or error
        self.assertIn(status, ["active", "waiting", "complete", "error", "paused", "removed"])

        # pause
        res = await self.rpc_call("pin.pause", [gid])
        self.assertNotIn("error", res, f"Error pausing: {res}")

        # unpause
        res = await self.rpc_call("pin.unpause", [gid])
        self.assertNotIn("error", res, f"Error unpausing: {res}")

        # changeOption
        res = await self.rpc_call("pin.changeOption", [gid, {"max-download-limit": "100K"}])
        self.assertNotIn("error", res, f"Error changing option: {res}")

        # getOption
        res = await self.rpc_call("pin.getOption", [gid])
        self.assertNotIn("error", res, f"Error getting option: {res}")
        self.assertIsInstance(res.get("result"), dict)

        # removeAndFile (if downloaded partially) or remove
        res = await self.rpc_call("pin.remove", [gid])
        self.assertNotIn("error", res, f"Error removing task: {res}")

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

    async def test_08_pause_resume_integrity(self):
        # Testing the pause/resume functionality to ensure no file corruption and no duplicate files are created
        test_out = "dummy_pause_resume.jpg"
        dest_path = os.path.join(TEMP_DIR, test_out)
        
        # Cleanup any existing files from previous runs
        for f in os.listdir(TEMP_DIR):
            if f.startswith("dummy_pause_resume"):
                try:
                    os.remove(os.path.join(TEMP_DIR, f))
                except OSError:
                    pass

        # Start download
        res = await self.rpc_call("pin.addUri", [
            [TEST_URL_2],
            {"dir": TEMP_DIR, "out": test_out, "split": "4"}
        ])
        gid = res.get("result")
        self.assertIsNotNone(gid, f"Failed to add URI, response: {res}")
        
        # Wait for download to start and get some chunks
        await asyncio.sleep(1)
        
        # Pause the download
        res = await self.rpc_call("pin.pause", [gid])
        self.assertNotIn("error", res, f"Failed to pause: {res}")
        
        # Verify status is paused
        res = await self.rpc_call("pin.tellStatus", [gid])
        self.assertEqual(res.get("result", {}).get("status"), "paused")
        
        # Unpause the download
        res = await self.rpc_call("pin.unpause", [gid])
        self.assertNotIn("error", res, f"Failed to unpause: {res}")
        
        # Wait for the download to complete
        for _ in range(15):
            res = await self.rpc_call("pin.tellStatus", [gid])
            if res.get("result", {}).get("status") == "complete":
                break
            await asyncio.sleep(1)
            
        res = await self.rpc_call("pin.tellStatus", [gid])
        self.assertEqual(res.get("result", {}).get("status"), "complete", "Download did not complete after unpause")
        
        # Verify the file exists and is not empty
        self.assertTrue(os.path.exists(dest_path), "Output file was not created.")
        self.assertGreater(os.path.getsize(dest_path), 0, "Output file is empty.")
        
        # Verify no duplicate files (e.g., dummy_pause_resume_1.jpg) were created
        duplicates = [f for f in os.listdir(TEMP_DIR) if f.startswith("dummy_pause_resume") and f != test_out]
        self.assertEqual(len(duplicates), 0, f"Duplicate files created during resume: {duplicates}")

if __name__ == "__main__":
    unittest.main()

