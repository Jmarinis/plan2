"""
MCP server for querying and controlling P2P mesh nodes.

Usage:
    P2P_NODES=http://10.0.0.1:3000,http://10.0.0.2:3000 python3 mcp_server.py
"""

import os
import json
import httpx
from typing import Any

from mcp.server import Server
from mcp.server.stdio import stdio_server
from mcp.types import (
    Tool,
    TextContent,
    ListToolsResult,
    CallToolResult,
)


P2P_NODES = os.environ.get("P2P_NODES", "").split(",")
P2P_NODES = [n.strip() for n in P2P_NODES if n.strip()]

if not P2P_NODES:
    print("Warning: P2P_NODES environment variable not set", file=__import__("sys").stderr)

server = Server("p2p-mesh")


async def api_get(node_url: str, path: str) -> dict[str, Any]:
    async with httpx.AsyncClient(timeout=10) as client:
        resp = await client.get(f"{node_url}{path}")
        resp.raise_for_status()
        return resp.json()


async def api_post(node_url: str, path: str, body: dict[str, Any]) -> dict[str, Any]:
    async with httpx.AsyncClient(timeout=10) as client:
        resp = await client.post(f"{node_url}{path}", json=body)
        return resp.json()


@server.list_tools()
async def list_tools() -> ListToolsResult:
    return ListToolsResult(tools=[
        Tool(
            name="get_nodes",
            description="List all configured P2P node URLs in the mesh",
            inputSchema={"type": "object", "properties": {}},
        ),
        Tool(
            name="get_status",
            description="Get detailed status of a P2P node (node info, connected peers, known peers, sessions)",
            inputSchema={
                "type": "object",
                "properties": {
                    "node_url": {
                        "type": "string",
                        "description": "URL of the P2P node (e.g. http://10.0.0.1:3000)",
                    },
                },
                "required": ["node_url"],
            },
        ),
        Tool(
            name="add_peer",
            description="Add and connect to a new peer on a specific node",
            inputSchema={
                "type": "object",
                "properties": {
                    "node_url": {
                        "type": "string",
                        "description": "URL of the P2P node",
                    },
                    "address": {
                        "type": "string",
                        "description": "IP address of the peer to add",
                    },
                    "port": {
                        "type": "integer",
                        "description": "Port of the peer to add",
                    },
                },
                "required": ["node_url", "address", "port"],
            },
        ),
        Tool(
            name="remove_peer",
            description="Remove a peer from a node's peer list",
            inputSchema={
                "type": "object",
                "properties": {
                    "node_url": {
                        "type": "string",
                        "description": "URL of the P2P node",
                    },
                    "peer_id": {
                        "type": "string",
                        "description": "ID of the peer to remove",
                    },
                },
                "required": ["node_url", "peer_id"],
            },
        ),
        Tool(
            name="disconnect_peer",
            description="Disconnect from a peer on a specific node",
            inputSchema={
                "type": "object",
                "properties": {
                    "node_url": {
                        "type": "string",
                        "description": "URL of the P2P node",
                    },
                    "peer_id": {
                        "type": "string",
                        "description": "ID of the peer to disconnect from",
                    },
                },
                "required": ["node_url", "peer_id"],
            },
        ),
        Tool(
            name="connect_peer",
            description="Attempt to reconnect to a disconnected peer on a specific node",
            inputSchema={
                "type": "object",
                "properties": {
                    "node_url": {
                        "type": "string",
                        "description": "URL of the P2P node",
                    },
                    "peer_id": {
                        "type": "string",
                        "description": "ID of the peer to connect to",
                    },
                },
                "required": ["node_url", "peer_id"],
            },
        ),
        Tool(
            name="refresh",
            description="Trigger a mesh-wide refresh (re-handshake with all connected peers) on a specific node",
            inputSchema={
                "type": "object",
                "properties": {
                    "node_url": {
                        "type": "string",
                        "description": "URL of the P2P node",
                    },
                },
                "required": ["node_url"],
            },
        ),
        Tool(
            name="mesh_query",
            description="Route an MCP query through the mesh. The entry node executes the tool locally, forwards to all connected peers, and aggregates responses from the full mesh topology.",
            inputSchema={
                "type": "object",
                "properties": {
                    "node_url": {
                        "type": "string",
                        "description": "URL of the entry P2P node to start the mesh query from",
                    },
                    "tool_name": {
                        "type": "string",
                        "description": "Name of the tool to execute across the mesh (e.g. get_status, refresh)",
                    },
                    "arguments": {
                        "type": "object",
                        "description": "Arguments for the tool (tool-specific)",
                    },
                    "hop_count": {
                        "type": "integer",
                        "description": "Maximum propagation depth (default 3)",
                        "default": 3,
                    },
                },
                "required": ["node_url", "tool_name"],
            },
        ),
    ])


@server.call_tool()
async def call_tool(name: str, arguments: dict[str, Any]) -> CallToolResult:
    try:
        if name == "get_nodes":
            return CallToolResult(content=[
                TextContent(type="text", text=json.dumps({
                    "nodes": P2P_NODES,
                    "count": len(P2P_NODES),
                }, indent=2)),
            ])

        node_url = arguments["node_url"].rstrip("/")

        if name == "get_status":
            data = await api_get(node_url, "/api/status")
            return CallToolResult(content=[
                TextContent(type="text", text=json.dumps(data, indent=2, default=str)),
            ])

        if name == "add_peer":
            result = await api_post(node_url, "/api/peers", {
                "address": arguments["address"],
                "port": arguments["port"],
            })
            return CallToolResult(content=[
                TextContent(type="text", text=json.dumps(result, indent=2, default=str)),
            ])

        if name == "remove_peer":
            result = await api_post(node_url, "/api/peers/remove", {
                "peer_id": arguments["peer_id"],
            })
            return CallToolResult(content=[
                TextContent(type="text", text=json.dumps(result, indent=2, default=str)),
            ])

        if name == "disconnect_peer":
            result = await api_post(node_url, "/api/peers/disconnect", {
                "peer_id": arguments["peer_id"],
            })
            return CallToolResult(content=[
                TextContent(type="text", text=json.dumps(result, indent=2, default=str)),
            ])

        if name == "connect_peer":
            result = await api_post(node_url, "/api/peers/connect", {
                "peer_id": arguments["peer_id"],
            })
            return CallToolResult(content=[
                TextContent(type="text", text=json.dumps(result, indent=2, default=str)),
            ])

        if name == "refresh":
            result = await api_post(node_url, "/api/refresh", {
                "request_id": __import__("uuid").uuid4().hex[:12],
            })
            return CallToolResult(content=[
                TextContent(type="text", text=json.dumps(result, indent=2, default=str)),
            ])

        if name == "mesh_query":
            tool_name = arguments["tool_name"]
            tool_args = arguments.get("arguments", {})
            hop_count = arguments.get("hop_count", 3)
            async with httpx.AsyncClient(timeout=30) as client:
                resp = await client.post(
                    f"{node_url}/api/mcp/query",
                    json={
                        "request_id": __import__("uuid").uuid4().hex,
                        "hop_count": hop_count,
                        "tool_name": tool_name,
                        "arguments": tool_args,
                    },
                )
                result = resp.json()
            return CallToolResult(content=[
                TextContent(type="text", text=json.dumps(result, indent=2, default=str)),
            ])

        return CallToolResult(content=[
            TextContent(type="text", text=f"Unknown tool: {name}"),
        ], isError=True)

    except httpx.HTTPStatusError as e:
        return CallToolResult(content=[
            TextContent(type="text", text=f"HTTP error from {node_url}: {e.response.status_code} {e.response.text}"),
        ], isError=True)
    except httpx.RequestError as e:
        return CallToolResult(content=[
            TextContent(type="text", text=f"Request failed to {node_url}: {e}"),
        ], isError=True)
    except Exception as e:
        return CallToolResult(content=[
            TextContent(type="text", text=f"Error: {e}"),
        ], isError=True)


async def main():
    init_options = server.create_initialization_options()
    async with stdio_server() as (read_stream, write_stream):
        await server.run(read_stream, write_stream, init_options)


if __name__ == "__main__":
    import anyio
    anyio.run(main)
