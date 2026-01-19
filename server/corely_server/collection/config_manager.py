"""Configuration manager for per-worker collection settings."""

import json
from typing import Optional
from datetime import datetime

from ..storage import storage
from ..worker_manager import worker_manager
from .models import CollectionConfig, CollectionStatus


class ConfigManager:
    """Manages collection configuration for workers."""

    async def get_config(self, worker_id: str) -> Optional[CollectionConfig]:
        """Get collection config for a worker."""
        result = await storage.get_collection_config(worker_id)
        if result:
            return CollectionConfig(**result["config"])
        return None

    async def set_config(
        self,
        worker_id: str,
        config: CollectionConfig,
        push_to_worker: bool = True,
    ) -> CollectionConfig:
        """Set collection config for a worker and optionally push to the worker."""
        # Apply default coupling rules
        config = config.apply_defaults()

        # Save to database
        await storage.upsert_collection_config(worker_id, config.model_dump())

        # Push to worker if online and requested
        if push_to_worker:
            await self.push_config_to_worker(worker_id, config)

        return config

    async def push_config_to_worker(
        self,
        worker_id: str,
        config: Optional[CollectionConfig] = None,
    ) -> bool:
        """Push configuration to a worker."""
        if config is None:
            config = await self.get_config(worker_id)
            if config is None:
                return False

        try:
            result = await worker_manager.call_worker(
                worker_id,
                "collection.update_config",
                config.model_dump(),
            )
            return result.get("status") == "ok"
        except Exception as e:
            # Worker may be offline
            return False

    async def start_collection(self, worker_id: str) -> dict:
        """Start collection on a worker."""
        result = await worker_manager.call_worker(
            worker_id,
            "collection.start",
            {},
        )

        if result.get("status") == "started":
            session_id = result.get("session_id")
            if session_id:
                await storage.create_collection_session(session_id, worker_id)

        return result

    async def stop_collection(self, worker_id: str) -> dict:
        """Stop collection on a worker."""
        # Get current status first to get session_id
        status = await self.get_status(worker_id)
        session_id = status.session_id if status else None

        result = await worker_manager.call_worker(
            worker_id,
            "collection.stop",
            {},
        )

        # End the session in database
        if session_id:
            chunk_count = status.chunk_count if status else 0
            await storage.end_collection_session(session_id, chunk_count)

        return result

    async def get_status(self, worker_id: str) -> Optional[CollectionStatus]:
        """Get collection status from a worker."""
        try:
            result = await worker_manager.call_worker(
                worker_id,
                "collection.status",
                {},
            )
            return CollectionStatus(
                is_collecting=result.get("is_collecting", False),
                session_id=result.get("session_id"),
                started_at=result.get("started_at"),
                ended_at=result.get("ended_at"),
                chunk_count=result.get("chunk_count", 0),
                active_streams=result.get("active_streams", []),
                last_error=result.get("last_error"),
            )
        except Exception as e:
            return CollectionStatus(
                is_collecting=False,
                last_error=str(e),
            )

    async def get_sessions(self, worker_id: str, limit: int = 50) -> list[dict]:
        """Get collection sessions for a worker."""
        return await storage.get_worker_sessions(worker_id, limit)

    async def get_session_chunks(self, session_id: str) -> list[dict]:
        """Get chunks for a session."""
        return await storage.get_session_chunks(session_id)

    async def set_encryption_key(self, worker_id: str, public_key: str):
        """Set encryption public key for a worker."""
        await storage.set_encryption_key(worker_id, public_key)

    async def set_use_infrequent_access(self, worker_id: str, use_ia: bool):
        """Set whether to use infrequent access storage for a worker."""
        config = await self.get_config(worker_id)
        if config:
            await storage.upsert_collection_config(
                worker_id,
                config.model_dump(),
                use_infrequent_access=use_ia,
            )


# Global instance
config_manager = ConfigManager()
