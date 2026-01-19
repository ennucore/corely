"""Worker connection management for Corely server."""

import asyncio
import json
import uuid
from dataclasses import dataclass, field
from datetime import datetime
from typing import Any, Callable, Optional

from fastapi import WebSocket
from pydantic import BaseModel

from .storage import storage


class WorkerInfo(BaseModel):
    id: str
    name: str
    hostname: Optional[str] = None
    os: Optional[str] = None
    arch: Optional[str] = None
    capabilities: list[str] = []
    connected_at: datetime = field(default_factory=datetime.utcnow)
    last_seen: datetime = field(default_factory=datetime.utcnow)


@dataclass
class WorkerConnection:
    worker_id: str
    websocket: WebSocket
    info: Optional[WorkerInfo] = None
    pending_requests: dict[str, asyncio.Future] = field(default_factory=dict)


class WorkerManager:
    def __init__(self):
        self._workers: dict[str, WorkerConnection] = {}
        self._lock = asyncio.Lock()

    async def register_worker(self, websocket: WebSocket) -> WorkerConnection:
        """Register a new worker connection with temporary ID."""
        # Use a temporary ID until we get the hello message with the real ID
        temp_id = f"pending-{uuid.uuid4()}"
        connection = WorkerConnection(worker_id=temp_id, websocket=websocket)

        async with self._lock:
            self._workers[temp_id] = connection

        return connection

    async def handle_hello(self, connection: WorkerConnection, params: dict):
        """Handle worker hello message with stable worker ID."""
        # Get the worker-provided ID (based on MAC address)
        worker_id = params.get("id")
        if not worker_id:
            # Fallback for old workers without stable ID
            worker_id = str(uuid.uuid4())

        old_id = connection.worker_id

        # Update the connection with the real worker ID
        async with self._lock:
            # Remove old temporary entry
            if old_id in self._workers:
                del self._workers[old_id]

            # Check if this worker is already connected (reconnection)
            if worker_id in self._workers:
                # Close the old connection
                old_conn = self._workers[worker_id]
                try:
                    await old_conn.websocket.close()
                except:
                    pass

            # Register with the real ID
            connection.worker_id = worker_id
            self._workers[worker_id] = connection

        connection.info = WorkerInfo(
            id=worker_id,
            name=params.get("name", "Unknown"),
            hostname=params.get("hostname"),
            os=params.get("os"),
            arch=params.get("arch"),
            capabilities=params.get("capabilities", []),
        )

        # Store in database (upsert to handle reconnections)
        await storage.upsert_worker(
            worker_id=worker_id,
            name=connection.info.name,
            hostname=connection.info.hostname,
            os=connection.info.os,
            arch=connection.info.arch,
            capabilities=connection.info.capabilities,
        )

        # Send back confirmation of the ID
        await self.send_message(
            connection,
            {
                "jsonrpc": "2.0",
                "method": "worker.set_id",
                "params": {"id": worker_id},
            },
        )

    async def unregister_worker(self, worker_id: str):
        """Unregister a worker connection."""
        async with self._lock:
            if worker_id in self._workers:
                connection = self._workers.pop(worker_id)
                # Cancel any pending requests
                for future in connection.pending_requests.values():
                    if not future.done():
                        future.cancel()
                # Mark offline in database
                await storage.set_worker_offline(worker_id)

    async def get_worker(self, worker_id: str) -> Optional[WorkerConnection]:
        """Get a worker connection by ID."""
        return self._workers.get(worker_id)

    async def get_all_workers(self) -> list[WorkerInfo]:
        """Get all connected workers."""
        workers = []
        for conn in self._workers.values():
            if conn.info:
                workers.append(conn.info)
        return workers

    async def send_message(self, connection: WorkerConnection, message: dict):
        """Send a message to a worker."""
        await connection.websocket.send_text(json.dumps(message))

    async def call_worker(
        self,
        worker_id: str,
        method: str,
        params: Optional[dict] = None,
        timeout: float = 300.0,
    ) -> Any:
        """Call a method on a worker and wait for the response."""
        connection = await self.get_worker(worker_id)
        if not connection:
            raise ValueError(f"Worker {worker_id} not found")

        request_id = str(uuid.uuid4())
        request = {
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params or {},
        }

        # Create a future to wait for the response
        future = asyncio.get_event_loop().create_future()
        connection.pending_requests[request_id] = future

        try:
            await self.send_message(connection, request)
            result = await asyncio.wait_for(future, timeout=timeout)
            return result
        finally:
            connection.pending_requests.pop(request_id, None)

    async def handle_response(self, connection: WorkerConnection, message: dict):
        """Handle a response from a worker."""
        request_id = message.get("id")
        if not request_id:
            return

        future = connection.pending_requests.get(request_id)
        if future and not future.done():
            if "error" in message:
                future.set_exception(
                    Exception(message["error"].get("message", "Unknown error"))
                )
            else:
                future.set_result(message.get("result"))


# Global worker manager instance
worker_manager = WorkerManager()
