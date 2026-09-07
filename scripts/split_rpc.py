import os
import re

TEST_RPC_FILE = "tests/test_rpc.py"
OUTPUT_DIR = "tests/rpc"

def extract_methods(content):
    methods = {}
    current_method = None
    current_lines = []
    
    for line in content.split('\n'):
        match = re.match(r'^    async def (test_\d+_[a-zA-Z0-9_]+)\(self\):', line)
        if match:
            if current_method:
                methods[current_method] = '\n'.join(current_lines)
            current_method = match.group(1)
            current_lines = [line]
        elif current_method:
            current_lines.append(line)
            
    if current_method:
        methods[current_method] = '\n'.join(current_lines)
        
    return methods

def write_test_file(filename, methods_dict, method_names):
    header = """import unittest
import websockets
import json
import asyncio
import os

TEMP_DIR = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "temporary")
TEST_URL_1 = "https://images.unsplash.com/photo-1777047023536-8e47688b77f9?ixlib=rb-4.1.0&q=85&fm=jpg&crop=entropy&cs=srgb&dl=nasa-JZz2UYtHo1s-unsplash.jpg"
TEST_URL_2 = "https://images.unsplash.com/photo-1614730321146-b6fa6a46bcb4?ixlib=rb-4.1.0&q=85&fm=jpg&crop=entropy&cs=srgb&dl=nasa-vhSz50AaFAs-unsplash.jpg"

class {class_name}(unittest.IsolatedAsyncioTestCase):
    URI = "ws://127.0.0.1:6842/jsonrpc"

    @classmethod
    def setUpClass(cls):
        os.makedirs(TEMP_DIR, exist_ok=True)

    async def rpc_call(self, method, params=None):
        req_id = f"test-{{method}}"
        if params is None:
            params = []
        payload = {{
            "jsonrpc": "2.0",
            "id": req_id,
            "method": method,
            "params": params
        }}
        try:
            async with websockets.connect(self.URI, open_timeout=5) as ws:
                await ws.send(json.dumps(payload))
                while True:
                    response = json.loads(await ws.recv())
                    if response.get("id") == req_id:
                        return response
        except Exception as e:
            self.fail(f"Failed to connect or communicate with RPC server: {{e}}")

"""
    class_name = "Test" + "".join(word.capitalize() for word in filename.replace("test_", "").replace(".py", "").split("_"))
    
    with open(os.path.join(OUTPUT_DIR, filename), "w") as f:
        f.write(header.format(class_name=class_name))
        for m in method_names:
            if m in methods_dict:
                f.write(methods_dict[m] + "\n")
        f.write("\nif __name__ == '__main__':\n    unittest.main()\n")

def main():
    with open(TEST_RPC_FILE, "r") as f:
        content = f.read()
        
    methods = extract_methods(content)
    
    # Groupings
    groups = {
        "test_lifecycle.py": ["test_03_lifecycle", "test_08_pause_resume_integrity"],
        "test_global_state.py": ["test_01_get_version", "test_02_global_options", "test_04_bulk_and_stats", "test_05_cleanup", "test_06_misc", "test_10_system_methods"],
        "test_auth.py": ["test_09_auth"],
        "test_advanced.py": ["test_11_task_introspection", "test_12_advanced_task_modification", "test_13_parameterized_uris"],
        "test_protocols.py": ["test_14_ftp_sftp", "test_15_bittorrent"],
        "test_security.py": ["test_07_path_traversal"]
    }
    
    for filename, method_names in groups.items():
        write_test_file(filename, methods, method_names)
        
if __name__ == "__main__":
    main()
