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
                    # Ignore async notifications and wait for our exact response
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
        res_status = await self.rpc_call("pin.tellStatus", [gid])
        self.assertEqual(res_status.get("result", {}).get("status"), "active")
        
        # Remove
        res_remove = await self.rpc_call("pin.remove", [gid])
        self.assertNotIn("error", res_remove)

if __name__ == "__main__":
    unittest.main()
