import asyncio
import websockets
import json

async def test_download():
    uri = "ws://127.0.0.1:6842/jsonrpc"
    async with websockets.connect(uri) as websocket:
        # Add URI
        payload = {
            "jsonrpc": "2.0",
            "id": "test-123",
            "method": "pin.addUri",
            "params": [
                ["https://videos.pexels.com/video-files/6133768/6133768-uhd_2160_4096_25fps.mp4"],
                {"dir": "/Users/mac/Downloads", "out": "test_video.mp4", "split": "4"}
            ]
        }
        await websocket.send(json.dumps(payload))
        response = await websocket.recv()
        print(f"Server response to addUri: {response}")

if __name__ == "__main__":
    asyncio.run(test_download())
