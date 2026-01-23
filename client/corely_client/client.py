"""Corely client implementation."""

import asyncio
import base64
from dataclasses import dataclass
from typing import Any, Optional
from pathlib import Path

import httpx
from pydantic import BaseModel


class Worker(BaseModel):
    """Worker information model."""

    id: str
    name: str
    hostname: Optional[str] = None
    os: Optional[str] = None
    arch: Optional[str] = None
    capabilities: list[str] = []
    is_online: bool = False
    last_seen: Optional[str] = None


class ShellResult(BaseModel):
    """Shell command result."""

    stdout: str
    stderr: str
    exit_code: int
    timed_out: bool = False


class FileContent(BaseModel):
    """File content result."""

    content: str
    total_lines: int
    offset: int
    lines_returned: int


class BinaryContent(BaseModel):
    """Binary file content result."""

    content: str  # base64 encoded
    size: int
    encoding: str


class GlobResult(BaseModel):
    """Glob search result."""

    matches: list[str]
    count: int


class GrepMatch(BaseModel):
    """Single grep match."""

    line: int
    content: str


class GrepFileResult(BaseModel):
    """Grep results for a single file."""

    file: str
    matches: list[GrepMatch]


class GrepResult(BaseModel):
    """Grep search result."""

    results: list[GrepFileResult]
    files_matched: int


class ScreenCapture(BaseModel):
    """Screen capture result."""

    width: int
    height: int
    format: str
    data: str  # base64 encoded


class SystemInfo(BaseModel):
    """System information."""

    hostname: str
    os: dict
    cpu: dict
    memory: dict
    swap: dict
    disks: list[dict]
    network: list[dict]
    uptime: int


class AsyncCorelyClient:
    """Async client for Corely server."""

    def __init__(
        self,
        base_url: str,
        username: Optional[str] = None,
        password: Optional[str] = None,
        token: Optional[str] = None,
    ):
        self.base_url = base_url.rstrip("/")
        self._token = token
        self._username = username
        self._password = password
        self._client = httpx.AsyncClient(base_url=self.base_url, timeout=300.0)

    async def __aenter__(self):
        if not self._token and self._username and self._password:
            await self.login(self._username, self._password)
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        await self._client.aclose()

    async def login(self, username: str, password: str) -> str:
        """Login and get access token."""
        response = await self._client.post(
            "/api/auth/login",
            data={"username": username, "password": password},
        )
        response.raise_for_status()
        data = response.json()
        self._token = data["access_token"]
        return self._token

    def _headers(self) -> dict:
        """Get request headers with auth."""
        headers = {}
        if self._token:
            headers["Authorization"] = f"Bearer {self._token}"
        return headers

    async def list_workers(self) -> list[Worker]:
        """List all workers."""
        response = await self._client.get("/api/workers", headers=self._headers())
        response.raise_for_status()
        data = response.json()
        return [Worker(**w) for w in data["workers"]]

    async def get_worker(self, worker_id: str) -> Worker:
        """Get a specific worker."""
        response = await self._client.get(
            f"/api/workers/{worker_id}", headers=self._headers()
        )
        response.raise_for_status()
        return Worker(**response.json())

    async def call(
        self,
        worker_id: str,
        method: str,
        params: Optional[dict] = None,
        timeout: float = 300.0,
    ) -> Any:
        """Call a method on a worker."""
        response = await self._client.post(
            f"/api/workers/{worker_id}/call",
            json={"method": method, "params": params or {}, "timeout": timeout},
            headers=self._headers(),
        )
        response.raise_for_status()
        return response.json()["result"]

    async def bash(
        self,
        worker_id: str,
        command: str,
        timeout: int = 30000,
        cwd: Optional[str] = None,
    ) -> ShellResult:
        """Execute a shell command on a worker."""
        result = await self.call(
            worker_id,
            "shell.exec",
            {"command": command, "timeout": timeout, "cwd": cwd},
        )
        return ShellResult(**result)

    async def read(
        self,
        worker_id: str,
        path: str,
        offset: Optional[int] = None,
        limit: Optional[int] = None,
    ) -> FileContent:
        """Read a file from a worker."""
        result = await self.call(
            worker_id,
            "fs.read",
            {"path": path, "offset": offset, "limit": limit},
        )
        return FileContent(**result)

    async def write(self, worker_id: str, path: str, content: str) -> dict:
        """Write content to a file on a worker."""
        return await self.call(
            worker_id,
            "fs.write",
            {"path": path, "content": content},
        )

    async def read_binary(self, worker_id: str, path: str) -> BinaryContent:
        """Read a binary file from a worker (base64 encoded)."""
        result = await self.call(
            worker_id,
            "fs.read_binary",
            {"path": path},
        )
        return BinaryContent(**result)

    async def write_binary(self, worker_id: str, path: str, content: str) -> dict:
        """Write binary content (base64-encoded) to a file on a worker."""
        return await self.call(
            worker_id,
            "fs.write_binary",
            {"path": path, "content": content},
        )

    async def edit(
        self, worker_id: str, path: str, old_string: str, new_string: str
    ) -> dict:
        """Edit a file on a worker."""
        return await self.call(
            worker_id,
            "fs.edit",
            {"path": path, "old_string": old_string, "new_string": new_string},
        )

    async def glob(
        self, worker_id: str, pattern: str, path: Optional[str] = None
    ) -> GlobResult:
        """Search for files matching a glob pattern."""
        result = await self.call(
            worker_id,
            "fs.glob",
            {"pattern": pattern, "path": path},
        )
        return GlobResult(**result)

    async def grep(
        self, worker_id: str, pattern: str, path: Optional[str] = None
    ) -> GrepResult:
        """Search file contents with regex."""
        result = await self.call(
            worker_id,
            "fs.grep",
            {"pattern": pattern, "path": path},
        )
        return GrepResult(**result)

    async def screenshot(
        self, worker_id: str, display_id: Optional[int] = None
    ) -> ScreenCapture:
        """Capture a screenshot from a worker."""
        result = await self.call(
            worker_id,
            "screen.capture",
            {"display_id": display_id},
        )
        return ScreenCapture(**result)

    async def save_screenshot(
        self,
        worker_id: str,
        output_path: str,
        display_id: Optional[int] = None,
    ):
        """Capture and save a screenshot to a file."""
        capture = await self.screenshot(worker_id, display_id)
        image_data = base64.b64decode(capture.data)
        Path(output_path).write_bytes(image_data)

    async def system_info(self, worker_id: str) -> SystemInfo:
        """Get system information from a worker."""
        result = await self.call(worker_id, "system.info", {})
        return SystemInfo(**result)

    async def mouse_move(self, worker_id: str, x: int, y: int) -> dict:
        """Move mouse cursor on a worker."""
        return await self.call(
            worker_id,
            "input.mouse_move",
            {"x": x, "y": y},
        )

    async def mouse_click(self, worker_id: str, button: str = "left") -> dict:
        """Click mouse on a worker."""
        return await self.call(
            worker_id,
            "input.mouse_click",
            {"button": button},
        )

    async def key_type(self, worker_id: str, text: str) -> dict:
        """Type text on a worker."""
        return await self.call(
            worker_id,
            "input.key_type",
            {"text": text},
        )

    async def key_press(
        self, worker_id: str, key: str, modifiers: Optional[list[str]] = None
    ) -> dict:
        """Press a key combination on a worker."""
        return await self.call(
            worker_id,
            "input.key_press",
            {"key": key, "modifiers": modifiers or []},
        )

    async def camera_capture(
        self, worker_id: str, device_index: Optional[int] = None
    ) -> ScreenCapture:
        """Capture from camera on a worker."""
        result = await self.call(
            worker_id,
            "camera.capture",
            {"device_index": device_index},
        )
        return ScreenCapture(**result)

    # Session (PTY) methods

    async def session_create(
        self, worker_id: str, name: Optional[str] = None, shell: Optional[str] = None
    ) -> dict:
        """Create a new terminal session on a worker."""
        return await self.call(
            worker_id,
            "session.create",
            {"name": name, "shell": shell},
        )

    async def session_list(self, worker_id: str) -> dict:
        """List all terminal sessions on a worker."""
        return await self.call(worker_id, "session.list", {})

    async def session_input(self, worker_id: str, session_id: str, input: str) -> dict:
        """Send input to a terminal session."""
        return await self.call(
            worker_id,
            "session.input",
            {"session_id": session_id, "input": input},
        )

    async def session_key(self, worker_id: str, session_id: str, key: str) -> dict:
        """Send a special key to a terminal session."""
        return await self.call(
            worker_id,
            "session.key",
            {"session_id": session_id, "key": key},
        )

    async def session_read(self, worker_id: str, session_id: str) -> dict:
        """Read output from a terminal session."""
        return await self.call(
            worker_id,
            "session.read",
            {"session_id": session_id},
        )

    async def session_resize(
        self, worker_id: str, session_id: str, cols: int = 120, rows: int = 40
    ) -> dict:
        """Resize a terminal session."""
        return await self.call(
            worker_id,
            "session.resize",
            {"session_id": session_id, "cols": cols, "rows": rows},
        )

    async def session_kill(self, worker_id: str, session_id: str) -> dict:
        """Kill a terminal session."""
        return await self.call(
            worker_id,
            "session.kill",
            {"session_id": session_id},
        )

    async def session_rename(
        self, worker_id: str, session_id: str, new_name: str
    ) -> dict:
        """Rename a terminal session."""
        return await self.call(
            worker_id,
            "session.rename",
            {"session_id": session_id, "name": new_name},
        )

    async def uninstall(self, worker_id: str) -> dict:
        """
        Completely uninstall Corely from a worker.

        This will:
        - Stop and remove all autostart configurations
        - Remove configuration files
        - Remove the worker binary
        - Terminate the worker process

        WARNING: This action is irreversible. The worker will disconnect
        and you will need to reinstall it to regain access.
        """
        return await self.call(worker_id, "system.uninstall", {})


class CorelyClient:
    """Synchronous wrapper for AsyncCorelyClient."""

    def __init__(
        self,
        base_url: str,
        username: Optional[str] = None,
        password: Optional[str] = None,
        token: Optional[str] = None,
    ):
        self._async_client = AsyncCorelyClient(
            base_url, username, password, token
        )
        self._loop = asyncio.new_event_loop()

    def __enter__(self):
        self._loop.run_until_complete(self._async_client.__aenter__())
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self._loop.run_until_complete(self._async_client.__aexit__(exc_type, exc_val, exc_tb))
        self._loop.close()

    def _run(self, coro):
        return self._loop.run_until_complete(coro)

    def login(self, username: str, password: str) -> str:
        return self._run(self._async_client.login(username, password))

    def list_workers(self) -> list[Worker]:
        return self._run(self._async_client.list_workers())

    def get_worker(self, worker_id: str) -> Worker:
        return self._run(self._async_client.get_worker(worker_id))

    def call(
        self,
        worker_id: str,
        method: str,
        params: Optional[dict] = None,
        timeout: float = 300.0,
    ) -> Any:
        return self._run(self._async_client.call(worker_id, method, params, timeout))

    def bash(
        self,
        worker_id: str,
        command: str,
        timeout: int = 30000,
        cwd: Optional[str] = None,
    ) -> ShellResult:
        return self._run(self._async_client.bash(worker_id, command, timeout, cwd))

    def read(
        self,
        worker_id: str,
        path: str,
        offset: Optional[int] = None,
        limit: Optional[int] = None,
    ) -> FileContent:
        return self._run(self._async_client.read(worker_id, path, offset, limit))

    def write(self, worker_id: str, path: str, content: str) -> dict:
        return self._run(self._async_client.write(worker_id, path, content))

    def read_binary(self, worker_id: str, path: str) -> BinaryContent:
        return self._run(self._async_client.read_binary(worker_id, path))

    def write_binary(self, worker_id: str, path: str, content: str) -> dict:
        return self._run(self._async_client.write_binary(worker_id, path, content))

    def edit(
        self, worker_id: str, path: str, old_string: str, new_string: str
    ) -> dict:
        return self._run(self._async_client.edit(worker_id, path, old_string, new_string))

    def glob(
        self, worker_id: str, pattern: str, path: Optional[str] = None
    ) -> GlobResult:
        return self._run(self._async_client.glob(worker_id, pattern, path))

    def grep(
        self, worker_id: str, pattern: str, path: Optional[str] = None
    ) -> GrepResult:
        return self._run(self._async_client.grep(worker_id, pattern, path))

    def screenshot(
        self, worker_id: str, display_id: Optional[int] = None
    ) -> ScreenCapture:
        return self._run(self._async_client.screenshot(worker_id, display_id))

    def save_screenshot(
        self,
        worker_id: str,
        output_path: str,
        display_id: Optional[int] = None,
    ):
        return self._run(self._async_client.save_screenshot(worker_id, output_path, display_id))

    def system_info(self, worker_id: str) -> SystemInfo:
        return self._run(self._async_client.system_info(worker_id))

    def mouse_move(self, worker_id: str, x: int, y: int) -> dict:
        return self._run(self._async_client.mouse_move(worker_id, x, y))

    def mouse_click(self, worker_id: str, button: str = "left") -> dict:
        return self._run(self._async_client.mouse_click(worker_id, button))

    def key_type(self, worker_id: str, text: str) -> dict:
        return self._run(self._async_client.key_type(worker_id, text))

    def key_press(
        self, worker_id: str, key: str, modifiers: Optional[list[str]] = None
    ) -> dict:
        return self._run(self._async_client.key_press(worker_id, key, modifiers))

    def camera_capture(
        self, worker_id: str, device_index: Optional[int] = None
    ) -> ScreenCapture:
        return self._run(self._async_client.camera_capture(worker_id, device_index))

    # Session (PTY) methods

    def session_create(
        self, worker_id: str, name: Optional[str] = None, shell: Optional[str] = None
    ) -> dict:
        return self._run(self._async_client.session_create(worker_id, name, shell))

    def session_list(self, worker_id: str) -> dict:
        return self._run(self._async_client.session_list(worker_id))

    def session_input(self, worker_id: str, session_id: str, input: str) -> dict:
        return self._run(self._async_client.session_input(worker_id, session_id, input))

    def session_key(self, worker_id: str, session_id: str, key: str) -> dict:
        return self._run(self._async_client.session_key(worker_id, session_id, key))

    def session_read(self, worker_id: str, session_id: str) -> dict:
        return self._run(self._async_client.session_read(worker_id, session_id))

    def session_resize(
        self, worker_id: str, session_id: str, cols: int = 120, rows: int = 40
    ) -> dict:
        return self._run(self._async_client.session_resize(worker_id, session_id, cols, rows))

    def session_kill(self, worker_id: str, session_id: str) -> dict:
        return self._run(self._async_client.session_kill(worker_id, session_id))

    def session_rename(
        self, worker_id: str, session_id: str, new_name: str
    ) -> dict:
        return self._run(self._async_client.session_rename(worker_id, session_id, new_name))

    def uninstall(self, worker_id: str) -> dict:
        """
        Completely uninstall Corely from a worker.

        WARNING: This action is irreversible.
        """
        return self._run(self._async_client.uninstall(worker_id))
