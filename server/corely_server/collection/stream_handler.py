"""WebSocket handler for receiving streaming data from workers."""

import asyncio
import struct
from pathlib import Path
from typing import Optional
from datetime import datetime
import uuid

from fastapi import WebSocket, WebSocketDisconnect

from ..storage import storage
from .cache_manager import cache_manager
from .models import StreamFrame


# Data type constants
DATA_TYPE_VIDEO = 0
DATA_TYPE_MIC = 1
DATA_TYPE_OUTPUT = 2
DATA_TYPE_INPUT = 3
DATA_TYPE_FILE = 4


class StreamHandler:
    """Handles incoming data streams from workers."""

    def __init__(self):
        self._active_streams: dict[str, WebSocket] = {}
        self._stream_metadata: dict[str, dict] = {}

    async def handle_websocket(
        self,
        websocket: WebSocket,
        worker_id: str,
        stream_type: str,
    ):
        """Handle a WebSocket connection for streaming data."""
        await websocket.accept()

        stream_key = f"{worker_id}:{stream_type}"
        self._active_streams[stream_key] = websocket
        self._stream_metadata[stream_key] = {
            "worker_id": worker_id,
            "stream_type": stream_type,
            "connected_at": datetime.utcnow().isoformat(),
            "frames_received": 0,
            "bytes_received": 0,
        }

        try:
            while True:
                # Receive binary frame
                data = await websocket.receive_bytes()
                await self._process_frame(worker_id, stream_type, data)

                # Update metadata
                self._stream_metadata[stream_key]["frames_received"] += 1
                self._stream_metadata[stream_key]["bytes_received"] += len(data)

        except WebSocketDisconnect:
            pass
        finally:
            del self._active_streams[stream_key]
            del self._stream_metadata[stream_key]

    async def _process_frame(
        self,
        worker_id: str,
        stream_type: str,
        data: bytes,
    ):
        """Process a single frame of data.

        Binary frame format:
        [0-7]   chunk_index (u64)
        [8-15]  timestamp_ms (u64)
        [16-19] data_type (u32)
        [20-23] data_length (u32)
        [24-]   payload
        """
        if len(data) < 24:
            return  # Invalid frame

        # Parse header
        chunk_index = struct.unpack("<Q", data[0:8])[0]
        timestamp_ms = struct.unpack("<Q", data[8:16])[0]
        data_type = struct.unpack("<I", data[16:20])[0]
        data_length = struct.unpack("<I", data[20:24])[0]

        # Extract payload
        payload = data[24:24 + data_length]

        # Determine file path based on data type
        type_name = self._get_type_name(data_type, stream_type)

        # Write to cache
        await cache_manager.write_chunk_data(
            worker_id=worker_id,
            chunk_index=chunk_index,
            stream_name=type_name,
            data=payload,
            timestamp_ms=timestamp_ms,
        )

    def _get_type_name(self, data_type: int, stream_type: str) -> str:
        """Get the stream name based on data type."""
        if data_type == DATA_TYPE_VIDEO:
            return f"{stream_type}/video.raw"
        elif data_type == DATA_TYPE_MIC:
            return "mic_audio.pcm"
        elif data_type == DATA_TYPE_OUTPUT:
            return "output_audio.pcm"
        elif data_type == DATA_TYPE_INPUT:
            return "input.log"
        elif data_type == DATA_TYPE_FILE:
            return f"files/{stream_type}"
        else:
            return f"unknown_{data_type}.bin"

    def get_active_streams(self) -> list[dict]:
        """Get list of active streams."""
        return [
            {
                "stream_key": key,
                **meta,
            }
            for key, meta in self._stream_metadata.items()
        ]

    def is_stream_active(self, worker_id: str, stream_type: str) -> bool:
        """Check if a stream is active."""
        stream_key = f"{worker_id}:{stream_type}"
        return stream_key in self._active_streams


class StreamBroadcaster:
    """Broadcasts stream data to web clients for live viewing."""

    def __init__(self):
        self._subscribers: dict[str, list[WebSocket]] = {}

    async def subscribe(self, worker_id: str, websocket: WebSocket):
        """Subscribe a web client to a worker's stream."""
        if worker_id not in self._subscribers:
            self._subscribers[worker_id] = []
        self._subscribers[worker_id].append(websocket)

    async def unsubscribe(self, worker_id: str, websocket: WebSocket):
        """Unsubscribe a web client from a worker's stream."""
        if worker_id in self._subscribers:
            self._subscribers[worker_id] = [
                ws for ws in self._subscribers[worker_id]
                if ws != websocket
            ]

    async def broadcast(self, worker_id: str, data: bytes):
        """Broadcast data to all subscribers of a worker's stream."""
        if worker_id not in self._subscribers:
            return

        disconnected = []
        for websocket in self._subscribers[worker_id]:
            try:
                await websocket.send_bytes(data)
            except Exception:
                disconnected.append(websocket)

        # Remove disconnected clients
        for ws in disconnected:
            await self.unsubscribe(worker_id, ws)


# Global instances
stream_handler = StreamHandler()
stream_broadcaster = StreamBroadcaster()
