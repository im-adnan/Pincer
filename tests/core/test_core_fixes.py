import unittest
import websockets
import json
import asyncio
import os
import shutil

TEST_IMAGE_URL = "https://images.pexels.com/photos/29422195/pexels-photo-29422195.jpeg?cs=srgb&dl=pexels-mehmet-demi-r-746820582-29422195.jpg&fm=jpg"
TEMP_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "temporary")

class TestCoreFixes(unittest.IsolatedAsyncioTestCase):
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

    async def test_01_http_partial_content_fallback(self):
        """
        Tests the HTTP range request fallback.
        Pexels returns 200 OK (ignores Range requests). The engine should detect this,
        abandon chunked downloading, and download it cleanly using a single stream, 
        rather than corrupting the file or failing.
        """
        out_name = "pexels_test_image.jpg"
        res = await self.rpc_call("pin.addUri", [
            [TEST_IMAGE_URL],
            {"dir": TEMP_DIR, "out": out_name, "split": "8"}
        ])
        gid = res.get("result")
        self.assertIsNotNone(gid, f"Failed to add URI, response: {res}")

        # Wait for the download to complete
        completed = False
        for _ in range(30):
            status_res = await self.rpc_call("pin.tellStatus", [gid])
            status = status_res.get("result", {}).get("status")
            if status == "complete":
                completed = True
                break
            elif status == "error":
                self.fail("Download failed with error state instead of falling back to single-thread.")
            await asyncio.sleep(1)

        self.assertTrue(completed, "Download timed out without completing.")

        # Verify the file was created and has non-zero size
        file_path = os.path.join(TEMP_DIR, out_name)
        self.assertTrue(os.path.exists(file_path), "Completed file is missing from disk.")
        self.assertGreater(os.path.getsize(file_path), 0, "File is empty.")

    async def test_02_deletion_parent_folder_protection(self):
        """
        Tests that when a task is removed, only the specific file is moved to trash.
        The parent directory must NOT be deleted.
        """
        test_subfolder = os.path.join(TEMP_DIR, "test_protection_folder")
        os.makedirs(test_subfolder, exist_ok=True)
        
        out_name = "dummy_to_delete.txt"
        file_path = os.path.join(test_subfolder, out_name)
        
        # Write a dummy file to simulate a completed task
        with open(file_path, "w") as f:
            f.write("dummy data")
            
        # Add a dummy task that points to this file
        res = await self.rpc_call("pin.addUri", [
            ["https://example.com/dummy.txt"],
            {"dir": test_subfolder, "out": out_name}
        ])
        gid = res.get("result")
        
        # Wait a moment for it to register, then force remove
        await asyncio.sleep(1)
        remove_res = await self.rpc_call("pin.remove", [gid])
        
        # Wait for deletion processing
        await asyncio.sleep(2)
        
        # The parent folder should STILL exist
        self.assertTrue(os.path.exists(test_subfolder), "CRITICAL: Parent folder was deleted during task removal!")
        
        # The file itself might still exist if it wasn't a completed internal task, 
        # but the key check is the parent directory integrity.

    async def test_03_query_parameter_extension_parsing(self):
        """
        Tests that query parameters do not confuse the extension parsing,
        causing unnecessary/failing file conversion attempts.
        """
        # A URL with query params
        test_url = "https://images.pexels.com/photos/29422195/pexels-photo-29422195.jpeg?cs=srgb&dl=pexels-mehmet-demi-r-746820582-29422195.jpg&fm=jpg"
        # No 'out' specified to force automatic extension detection
        res = await self.rpc_call("pin.addUri", [
            [test_url],
            {"dir": TEMP_DIR}
        ])
        gid = res.get("result")
        self.assertIsNotNone(gid, f"Failed to add URI, response: {res}")

        completed = False
        final_status = None
        for _ in range(30):
            status_res = await self.rpc_call("pin.tellStatus", [gid])
            status = status_res.get("result", {}).get("status")
            if status == "complete":
                completed = True
                break
            elif status == "error":
                final_status = status_res
                break
            await asyncio.sleep(1)

        self.assertTrue(completed, f"Task did not complete successfully. Status: {final_status}")

    async def test_04_format_conversion_graceful_fallback(self):
        """
        Tests that if format conversion fails (e.g., unsupported format),
        it gracefully restores the original file rather than failing the task.
        """
        out_name = "test_graceful_fallback.jpg"
        res = await self.rpc_call("pin.addUri", [
            [TEST_IMAGE_URL],
            # Attempt to convert to a nonsense format
            {"dir": TEMP_DIR, "out": out_name, "format": "nonsenseformat"}
        ])
        gid = res.get("result")
        self.assertIsNotNone(gid, f"Failed to add URI, response: {res}")

        completed = False
        for _ in range(30):
            status_res = await self.rpc_call("pin.tellStatus", [gid])
            status = status_res.get("result", {}).get("status")
            if status == "complete":
                completed = True
                break
            elif status == "error":
                self.fail("Task errored out instead of falling back on conversion failure.")
            await asyncio.sleep(1)

        self.assertTrue(completed, "Download timed out without completing.")
        
        # Verify the original file still exists
        file_path = os.path.join(TEMP_DIR, out_name)
        self.assertTrue(os.path.exists(file_path), "Original file missing after failed conversion.")
        self.assertGreater(os.path.getsize(file_path), 0, "Original file is empty.")
