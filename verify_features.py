import asyncio
import websockets
import json
import time

async def test_rpc():
    uri = "ws://127.0.0.1:6842/jsonrpc"
    try:
        async with websockets.connect(uri) as websocket:
            print("Connected to Pincer RPC")

            # 1. Add Task
            add_payload = {
                "jsonrpc": "2.0",
                "id": "add-1",
                "method": "pin.addUri",
                "params": [
                    ["https://raw.githubusercontent.com/agalwood/App/master/LICENSE"],
                    {"dir": "./", "out": "LICENSE_TEST", "split": "1"}
                ]
            }
            await websocket.send(json.dumps(add_payload))
            res = json.loads(await websocket.recv())
            gid = res.get("result")
            print(f"Task added, GID: {gid}")

            # 2. Tell Status
            status_payload = {
                "jsonrpc": "2.0",
                "id": "status-1",
                "method": "pin.tellStatus",
                "params": [gid]
            }
            await websocket.send(json.dumps(status_payload))
            res = json.loads(await websocket.recv())
            print(f"Task status: {res.get('result', {}).get('status')}")

            # 3. Pause
            pause_payload = {
                "jsonrpc": "2.0",
                "id": "pause-1",
                "method": "pin.pause",
                "params": [gid]
            }
            await websocket.send(json.dumps(pause_payload))
            res = json.loads(await websocket.recv())
            print(f"Pause result: {res.get('result')}")

            # 4. Remove
            remove_payload = {
                "jsonrpc": "2.0",
                "id": "remove-1",
                "method": "pin.remove",
                "params": [gid]
            }
            await websocket.send(json.dumps(remove_payload))
            res = json.loads(await websocket.recv())
            print(f"Remove result: {res.get('result')}")

    except Exception as e:
        print(f"Error: {e}")

if __name__ == "__main__":
    asyncio.run(test_rpc())
