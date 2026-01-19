"""Local cache manager with LRU eviction for collected data."""

import asyncio
import os
import shutil
from pathlib import Path
from typing import Optional
from datetime import datetime
from collections import OrderedDict

from ..storage import storage


# Default cache size: 40GB
DEFAULT_CACHE_SIZE = 40 * 1024 * 1024 * 1024

# Default cache directory
DEFAULT_CACHE_DIR = "/tmp/corely_cache"


class CacheManager:
    """Manages local cache with LRU eviction."""

    def __init__(
        self,
        cache_dir: str = DEFAULT_CACHE_DIR,
        max_size: int = DEFAULT_CACHE_SIZE,
    ):
        self.cache_dir = Path(cache_dir)
        self.max_size = max_size
        self.current_size = 0

        # LRU tracking: chunk_id -> (path, size, last_access_time)
        self._lru: OrderedDict[str, tuple[Path, int, float]] = OrderedDict()

        # Lock for cache operations
        self._lock = asyncio.Lock()

        # Ensure cache directory exists
        self.cache_dir.mkdir(parents=True, exist_ok=True)

    async def init(self):
        """Initialize cache by scanning existing files."""
        async with self._lock:
            self.current_size = 0
            self._lru.clear()

            # Scan cache directory
            for worker_dir in self.cache_dir.iterdir():
                if not worker_dir.is_dir():
                    continue
                for session_dir in worker_dir.iterdir():
                    if not session_dir.is_dir():
                        continue
                    for chunk_dir in session_dir.iterdir():
                        if not chunk_dir.is_dir():
                            continue
                        size = self._get_dir_size(chunk_dir)
                        chunk_id = f"{worker_dir.name}/{session_dir.name}/{chunk_dir.name}"
                        self._lru[chunk_id] = (chunk_dir, size, chunk_dir.stat().st_mtime)
                        self.current_size += size

    def _get_dir_size(self, path: Path) -> int:
        """Get total size of a directory."""
        total = 0
        for item in path.rglob("*"):
            if item.is_file():
                total += item.stat().st_size
        return total

    async def write_chunk_data(
        self,
        worker_id: str,
        chunk_index: int,
        stream_name: str,
        data: bytes,
        timestamp_ms: int,
        session_id: Optional[str] = None,
    ) -> Path:
        """Write data to a chunk in the cache."""
        async with self._lock:
            # Determine session ID (use current date if not provided)
            if session_id is None:
                session_id = datetime.utcnow().strftime("%Y%m%d")

            # Construct path
            chunk_dir = self.cache_dir / worker_id / session_id / f"chunk_{chunk_index:05d}"
            chunk_dir.mkdir(parents=True, exist_ok=True)

            # Write data
            file_path = chunk_dir / stream_name
            file_path.parent.mkdir(parents=True, exist_ok=True)

            # Append to file
            with open(file_path, "ab") as f:
                f.write(data)

            # Update LRU tracking
            chunk_id = f"{worker_id}/{session_id}/chunk_{chunk_index:05d}"
            size = self._get_dir_size(chunk_dir)

            old_size = 0
            if chunk_id in self._lru:
                old_size = self._lru[chunk_id][1]
                self._lru.move_to_end(chunk_id)

            self._lru[chunk_id] = (chunk_dir, size, datetime.utcnow().timestamp())
            self.current_size += (size - old_size)

            # Evict if over limit
            await self._evict_if_needed()

            return file_path

    async def _evict_if_needed(self):
        """Evict old chunks if cache is over limit."""
        while self.current_size > self.max_size and self._lru:
            # Get oldest item
            chunk_id, (path, size, _) = self._lru.popitem(last=False)

            # Check if chunk has been uploaded to R2
            chunk_record = await storage.get_chunk(chunk_id)
            if chunk_record and chunk_record.get("r2_path"):
                # Safe to delete locally
                try:
                    shutil.rmtree(path)
                    self.current_size -= size
                    await storage.clear_chunk_local_path(chunk_id)
                except Exception:
                    pass
            else:
                # Not yet uploaded, put back in LRU (at the end)
                self._lru[chunk_id] = (path, size, datetime.utcnow().timestamp())

    async def get_chunk_path(self, worker_id: str, session_id: str, chunk_index: int) -> Optional[Path]:
        """Get path to a chunk if it exists in cache."""
        chunk_dir = self.cache_dir / worker_id / session_id / f"chunk_{chunk_index:05d}"
        if chunk_dir.exists():
            # Update LRU
            chunk_id = f"{worker_id}/{session_id}/chunk_{chunk_index:05d}"
            if chunk_id in self._lru:
                async with self._lock:
                    self._lru.move_to_end(chunk_id)
            return chunk_dir
        return None

    async def get_chunk_file(
        self,
        worker_id: str,
        session_id: str,
        chunk_index: int,
        stream_name: str,
    ) -> Optional[Path]:
        """Get path to a specific file in a chunk."""
        chunk_dir = await self.get_chunk_path(worker_id, session_id, chunk_index)
        if chunk_dir:
            file_path = chunk_dir / stream_name
            if file_path.exists():
                return file_path
        return None

    async def delete_chunk(self, worker_id: str, session_id: str, chunk_index: int):
        """Delete a chunk from cache."""
        async with self._lock:
            chunk_id = f"{worker_id}/{session_id}/chunk_{chunk_index:05d}"
            if chunk_id in self._lru:
                path, size, _ = self._lru.pop(chunk_id)
                try:
                    shutil.rmtree(path)
                    self.current_size -= size
                except Exception:
                    pass

    async def get_stats(self) -> dict:
        """Get cache statistics."""
        return {
            "cache_dir": str(self.cache_dir),
            "max_size_bytes": self.max_size,
            "current_size_bytes": self.current_size,
            "usage_percent": (self.current_size / self.max_size * 100) if self.max_size > 0 else 0,
            "chunk_count": len(self._lru),
        }

    async def list_chunks(self, worker_id: Optional[str] = None) -> list[dict]:
        """List chunks in cache."""
        chunks = []
        for chunk_id, (path, size, last_access) in self._lru.items():
            if worker_id and not chunk_id.startswith(f"{worker_id}/"):
                continue
            parts = chunk_id.split("/")
            chunks.append({
                "chunk_id": chunk_id,
                "worker_id": parts[0] if len(parts) > 0 else None,
                "session_id": parts[1] if len(parts) > 1 else None,
                "chunk_name": parts[2] if len(parts) > 2 else None,
                "size_bytes": size,
                "last_access": datetime.fromtimestamp(last_access).isoformat(),
                "path": str(path),
            })
        return chunks


# Global instance
cache_manager = CacheManager()
