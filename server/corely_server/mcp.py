"""MCP (Model Context Protocol) server implementation for Corely."""

import asyncio
import json
from typing import Any, Sequence

from mcp.server import Server
from mcp.server.stdio import stdio_server
from mcp.types import (
    CallToolResult,
    TextContent,
    Tool,
    EmbeddedResource,
)

from .worker_manager import worker_manager
from .storage import storage

# Create MCP server instance
mcp_server = Server("corely")


def get_worker_tools() -> list[Tool]:
    """Generate tools for all connected workers."""
    tools = [
        Tool(
            name="list_workers",
            description="List all connected workers with their status and info",
            inputSchema={
                "type": "object",
                "properties": {},
            },
        ),
    ]

    # For each connected worker, generate worker-specific tools
    # Note: In actual usage, this is called dynamically
    return tools


async def get_dynamic_tools() -> list[Tool]:
    """Get tools including dynamic per-worker tools."""
    tools = [
        Tool(
            name="list_workers",
            description="List all connected Corely workers with their status, hostname, OS, and capabilities",
            inputSchema={
                "type": "object",
                "properties": {},
            },
        ),
    ]

    # Get connected workers
    workers = await worker_manager.get_all_workers()

    for worker in workers:
        worker_id = worker.id
        worker_name = worker.name or worker_id[:8]
        prefix = f"{worker_id}"

        # Shell execution
        tools.append(
            Tool(
                name=f"{prefix}_bash",
                description=f"Execute a shell command on worker '{worker_name}' ({worker.hostname or 'unknown host'})",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The shell command to execute",
                        },
                        "timeout": {
                            "type": "integer",
                            "description": "Timeout in milliseconds (default: 30000)",
                        },
                        "cwd": {
                            "type": "string",
                            "description": "Working directory for the command",
                        },
                    },
                    "required": ["command"],
                },
            )
        )

        # File read
        tools.append(
            Tool(
                name=f"{prefix}_read",
                description=f"Read a file from worker '{worker_name}'",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to read",
                        },
                        "offset": {
                            "type": "integer",
                            "description": "Line offset to start reading from",
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of lines to read",
                        },
                    },
                    "required": ["path"],
                },
            )
        )

        # File write
        tools.append(
            Tool(
                name=f"{prefix}_write",
                description=f"Write content to a file on worker '{worker_name}'",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to write",
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to write to the file",
                        },
                    },
                    "required": ["path", "content"],
                },
            )
        )

        # File edit
        tools.append(
            Tool(
                name=f"{prefix}_edit",
                description=f"Edit a file on worker '{worker_name}' by replacing a string",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Path to the file to edit",
                        },
                        "old_string": {
                            "type": "string",
                            "description": "String to find and replace (must be unique in file)",
                        },
                        "new_string": {
                            "type": "string",
                            "description": "Replacement string",
                        },
                    },
                    "required": ["path", "old_string", "new_string"],
                },
            )
        )

        # Glob search
        tools.append(
            Tool(
                name=f"{prefix}_glob",
                description=f"Search for files matching a glob pattern on worker '{worker_name}'",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Glob pattern (e.g., '**/*.py')",
                        },
                        "path": {
                            "type": "string",
                            "description": "Base directory for search",
                        },
                    },
                    "required": ["pattern"],
                },
            )
        )

        # Grep search
        tools.append(
            Tool(
                name=f"{prefix}_grep",
                description=f"Search file contents with regex on worker '{worker_name}'",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Regex pattern to search for",
                        },
                        "path": {
                            "type": "string",
                            "description": "Directory to search in",
                        },
                    },
                    "required": ["pattern"],
                },
            )
        )

        # Screen capture
        tools.append(
            Tool(
                name=f"{prefix}_screenshot",
                description=f"Capture a screenshot from worker '{worker_name}'",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "display_id": {
                            "type": "integer",
                            "description": "Display ID to capture (default: primary)",
                        },
                    },
                },
            )
        )

        # System info
        tools.append(
            Tool(
                name=f"{prefix}_system_info",
                description=f"Get system information from worker '{worker_name}' (CPU, memory, disk, network)",
                inputSchema={
                    "type": "object",
                    "properties": {},
                },
            )
        )

        # Mouse move
        tools.append(
            Tool(
                name=f"{prefix}_mouse_move",
                description=f"Move the mouse cursor on worker '{worker_name}'",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "x": {"type": "integer", "description": "X coordinate"},
                        "y": {"type": "integer", "description": "Y coordinate"},
                    },
                    "required": ["x", "y"],
                },
            )
        )

        # Mouse click
        tools.append(
            Tool(
                name=f"{prefix}_mouse_click",
                description=f"Click the mouse on worker '{worker_name}'",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "button": {
                            "type": "string",
                            "enum": ["left", "right", "middle"],
                            "description": "Mouse button to click",
                        },
                    },
                },
            )
        )

        # Key type
        tools.append(
            Tool(
                name=f"{prefix}_key_type",
                description=f"Type text on worker '{worker_name}'",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "Text to type",
                        },
                    },
                    "required": ["text"],
                },
            )
        )

        # Key press
        tools.append(
            Tool(
                name=f"{prefix}_key_press",
                description=f"Press a key combination on worker '{worker_name}'",
                inputSchema={
                    "type": "object",
                    "properties": {
                        "key": {
                            "type": "string",
                            "description": "Key to press (e.g., 'enter', 'tab', 'a')",
                        },
                        "modifiers": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Modifier keys (e.g., ['ctrl', 'shift'])",
                        },
                    },
                    "required": ["key"],
                },
            )
        )

    return tools


@mcp_server.list_tools()
async def list_tools() -> list[Tool]:
    """List available tools."""
    return await get_dynamic_tools()


async def call_tool(name: str, arguments: dict[str, Any]) -> Sequence[TextContent]:
    """Handle tool calls - standalone function for HTTP API."""
    return await _handle_tool_call(name, arguments)


@mcp_server.call_tool()
async def _mcp_call_tool(name: str, arguments: dict[str, Any]) -> Sequence[TextContent]:
    """Handle tool calls - MCP decorator wrapper."""
    return await _handle_tool_call(name, arguments)


async def _handle_tool_call(name: str, arguments: dict[str, Any]) -> Sequence[TextContent]:
    """Handle tool calls - internal implementation."""

    if name == "list_workers":
        workers = await worker_manager.get_all_workers()
        db_workers = await storage.get_all_workers()

        # Merge connected and db info
        result = []
        connected_ids = {w.id for w in workers}

        for w in db_workers:
            info = {
                "id": w["id"],
                "name": w["name"],
                "hostname": w["hostname"],
                "os": w["os"],
                "arch": w["arch"],
                "is_online": w["id"] in connected_ids,
                "last_seen": w["last_seen"],
            }
            result.append(info)

        return [TextContent(type="text", text=json.dumps(result, indent=2))]

    # Parse worker ID from tool name
    parts = name.rsplit("_", 1)
    if len(parts) != 2:
        return [TextContent(type="text", text=f"Unknown tool: {name}")]

    worker_id = parts[0]
    action = parts[1]

    # Map action to method
    method_map = {
        "bash": ("shell.exec", lambda a: {
            "command": a["command"],
            "timeout": a.get("timeout", 30000),
            "cwd": a.get("cwd"),
        }),
        "read": ("fs.read", lambda a: {
            "path": a["path"],
            "offset": a.get("offset"),
            "limit": a.get("limit"),
        }),
        "write": ("fs.write", lambda a: {
            "path": a["path"],
            "content": a["content"],
        }),
        "edit": ("fs.edit", lambda a: {
            "path": a["path"],
            "old_string": a["old_string"],
            "new_string": a["new_string"],
        }),
        "glob": ("fs.glob", lambda a: {
            "pattern": a["pattern"],
            "path": a.get("path"),
        }),
        "grep": ("fs.grep", lambda a: {
            "pattern": a["pattern"],
            "path": a.get("path"),
        }),
        "screenshot": ("screen.capture", lambda a: {
            "display_id": a.get("display_id"),
        }),
        "info": ("system.info", lambda a: {}),
        "move": ("input.mouse_move", lambda a: {
            "x": a["x"],
            "y": a["y"],
        }),
        "click": ("input.mouse_click", lambda a: {
            "button": a.get("button", "left"),
        }),
        "type": ("input.key_type", lambda a: {
            "text": a["text"],
        }),
        "press": ("input.key_press", lambda a: {
            "key": a["key"],
            "modifiers": a.get("modifiers", []),
        }),
    }

    if action not in method_map:
        return [TextContent(type="text", text=f"Unknown action: {action}")]

    method, param_builder = method_map[action]

    try:
        params = param_builder(arguments)
        result = await worker_manager.call_worker(worker_id, method, params)
        return [TextContent(type="text", text=json.dumps(result, indent=2))]
    except Exception as e:
        return [TextContent(type="text", text=f"Error: {str(e)}")]


async def run_mcp_server():
    """Run the MCP server over stdio."""
    async with stdio_server() as (read_stream, write_stream):
        await mcp_server.run(read_stream, write_stream, mcp_server.create_initialization_options())
