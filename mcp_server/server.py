import os
import json
import urllib.request
import urllib.error
from typing import Any
from mcp.server import Server, NotificationOptions
from mcp.server.models import InitializationOptions
from mcp.types import (
    Tool,
    TextContent,
    ImageContent,
    EmbeddedResource,
)
import mcp.server.stdio

P2P_URL = os.environ.get("P2P_URL", "http://127.0.0.1:3000")


def _req(method: str, path: str, body: dict | None = None) -> dict | list:
    url = f"{P2P_URL}{path}"
    data = json.dumps(body).encode() if body else None
    req = urllib.request.Request(
        url,
        data=data,
        method=method,
        headers={"Content-Type": "application/json"} if body else {},
    )
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read().decode())
    except urllib.error.HTTPError as e:
        return {"error": f"HTTP {e.code}: {e.read().decode()}"}
    except urllib.error.URLError as e:
        return {"error": f"Connection failed: {e.reason}"}


async def serve() -> None:
    server = Server("p2p-mesh")

    @server.list_tools()
    async def list_tools() -> list[Tool]:
        return [
            Tool(
                name="get_status",
                description="Get the status of the local P2P node, including connected peers and known peers",
                inputSchema={"type": "object", "properties": {}},
            ),
            Tool(
                name="refresh",
                description="Trigger a mesh refresh — re-handshake with all connected peers to exchange known peer lists",
                inputSchema={"type": "object", "properties": {}},
            ),
            Tool(
                name="update",
                description="Trigger an update on the local node — downloads the latest binary from GitHub Releases and restarts",
                inputSchema={"type": "object", "properties": {}},
            ),
            Tool(
                name="add_peer",
                description="Connect to a new peer by address and port",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "address": {"type": "string", "description": "IP address of the peer"},
                        "port": {"type": "integer", "description": "Port of the peer"},
                    },
                    "required": ["address", "port"],
                },
            ),
            Tool(
                name="remove_peer",
                description="Remove a known peer by peer ID or address:port",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "peer_id": {"type": "string", "description": "Peer ID to remove"},
                        "address": {"type": "string", "description": "Fallback: peer address"},
                        "port": {"type": "integer", "description": "Fallback: peer port"},
                    },
                    "required": [],
                },
            ),
            Tool(
                name="disconnect_peer",
                description="Disconnect from a connected peer by peer ID or address:port",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "peer_id": {"type": "string", "description": "Peer ID to disconnect"},
                        "address": {"type": "string", "description": "Fallback: peer address"},
                        "port": {"type": "integer", "description": "Fallback: peer port"},
                    },
                    "required": [],
                },
            ),
            Tool(
                name="query_mesh",
                description="Execute a mesh-wide MCP query. The tool runs locally and forwards to connected peers up to hop_count hops",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "tool_name": {
                            "type": "string",
                            "description": "Tool to execute on each node (e.g. get_status, refresh, update)",
                            "enum": ["get_status", "refresh", "update"],
                        },
                        "hop_count": {
                            "type": "integer",
                            "description": "How many hops to forward the query (0 = local only)",
                            "default": 1,
                        },
                    },
                    "required": ["tool_name"],
                },
            ),
        ]

    @server.call_tool()
    async def call_tool(name: str, arguments: dict) -> list[TextContent | ImageContent | EmbeddedResource]:
        match name:
            case "get_status":
                result = _req("GET", "/api/status")

            case "refresh":
                import uuid
                result = _req("POST", "/api/refresh", {"request_id": str(uuid.uuid4())})

            case "update":
                result = _req("GET", "/api/update")

            case "add_peer":
                addr = arguments.get("address")
                port = arguments.get("port")
                if not addr or not port:
                    return [TextContent(type="text", text="Missing required arguments: address, port")]
                result = _req("POST", "/api/peers", {"address": addr, "port": port})

            case "remove_peer":
                payload: dict[str, Any] = {"peer_id": arguments.get("peer_id", "")}
                if arguments.get("address"):
                    payload["address"] = arguments["address"]
                if arguments.get("port"):
                    payload["port"] = arguments["port"]
                result = _req("POST", "/api/peers/remove", payload)

            case "disconnect_peer":
                payload: dict[str, Any] = {"peer_id": arguments.get("peer_id", "")}
                if arguments.get("address"):
                    payload["address"] = arguments["address"]
                if arguments.get("port"):
                    payload["port"] = arguments["port"]
                result = _req("POST", "/api/peers/disconnect", payload)

            case "query_mesh":
                import uuid
                tool_name = arguments.get("tool_name")
                if not tool_name:
                    return [TextContent(type="text", text="Missing required argument: tool_name")]
                hop_count = arguments.get("hop_count", 1)
                result = _req("POST", "/api/mcp/query", {
                    "request_id": str(uuid.uuid4()),
                    "hop_count": hop_count,
                    "tool_name": tool_name,
                    "arguments": {},
                })

            case _:
                return [TextContent(type="text", text=f"Unknown tool: {name}")]

        text = json.dumps(result, indent=2, default=str)
        return [TextContent(type="text", text=text)]

    async with mcp.server.stdio.stdio_server() as (read_stream, write_stream):
        await server.run(
            read_stream,
            write_stream,
            InitializationOptions(
                server_name="p2p-mesh",
                server_version="0.1.0",
            ),
        )


if __name__ == "__main__":
    import asyncio
    asyncio.run(serve())
