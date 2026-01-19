"""SQLite storage for Corely server."""

import aiosqlite
from datetime import datetime
from pathlib import Path
from typing import Optional
import json


class Storage:
    def __init__(self, db_path: str = "corely.db"):
        self.db_path = db_path
        self._db: Optional[aiosqlite.Connection] = None

    async def init(self):
        """Initialize the database."""
        self._db = await aiosqlite.connect(self.db_path)
        self._db.row_factory = aiosqlite.Row

        await self._db.executescript(
            """
            CREATE TABLE IF NOT EXISTS workers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                hostname TEXT,
                os TEXT,
                arch TEXT,
                capabilities TEXT,
                first_seen TEXT NOT NULL,
                last_seen TEXT NOT NULL,
                is_online INTEGER DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS users (
                username TEXT PRIMARY KEY,
                hashed_password TEXT NOT NULL,
                disabled INTEGER DEFAULT 0,
                scopes TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                worker_id TEXT NOT NULL,
                user TEXT NOT NULL,
                started_at TEXT NOT NULL,
                ended_at TEXT,
                FOREIGN KEY (worker_id) REFERENCES workers(id)
            );

            CREATE TABLE IF NOT EXISTS oauth_clients (
                client_id TEXT PRIMARY KEY,
                client_secret_hash TEXT NOT NULL,
                name TEXT NOT NULL,
                scopes TEXT NOT NULL,
                created_at TEXT NOT NULL,
                created_by TEXT NOT NULL,
                last_used TEXT,
                is_active INTEGER DEFAULT 1
            );

            -- Per-worker collection configuration
            CREATE TABLE IF NOT EXISTS collection_configs (
                worker_id TEXT PRIMARY KEY,
                config_json TEXT NOT NULL,
                encryption_public_key TEXT,
                use_infrequent_access INTEGER DEFAULT 0,
                updated_at TEXT NOT NULL
            );

            -- Collection session tracking
            CREATE TABLE IF NOT EXISTS collection_sessions (
                session_id TEXT PRIMARY KEY,
                worker_id TEXT NOT NULL,
                started_at TEXT,
                ended_at TEXT,
                status TEXT DEFAULT 'active',
                total_chunks INTEGER DEFAULT 0,
                FOREIGN KEY (worker_id) REFERENCES workers(id)
            );

            -- Chunk metadata
            CREATE TABLE IF NOT EXISTS collection_chunks (
                chunk_id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                worker_id TEXT NOT NULL,
                chunk_index INTEGER,
                start_timestamp INTEGER,
                end_timestamp INTEGER,
                local_path TEXT,
                r2_path TEXT,
                size_bytes INTEGER,
                encrypted INTEGER DEFAULT 0,
                status TEXT DEFAULT 'recording',
                FOREIGN KEY (session_id) REFERENCES collection_sessions(session_id),
                FOREIGN KEY (worker_id) REFERENCES workers(id)
            );

            -- R2 storage credentials (encrypted)
            CREATE TABLE IF NOT EXISTS r2_config (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                endpoint TEXT,
                access_key_encrypted TEXT,
                secret_key_encrypted TEXT,
                bucket_normal TEXT,
                bucket_infrequent TEXT
            );
        """
        )
        await self._db.commit()

    async def close(self):
        """Close the database connection."""
        if self._db:
            await self._db.close()

    async def upsert_worker(
        self,
        worker_id: str,
        name: str,
        hostname: Optional[str] = None,
        os: Optional[str] = None,
        arch: Optional[str] = None,
        capabilities: Optional[list] = None,
    ):
        """Insert or update a worker."""
        now = datetime.utcnow().isoformat()
        caps_json = json.dumps(capabilities or [])

        await self._db.execute(
            """
            INSERT INTO workers (id, name, hostname, os, arch, capabilities, first_seen, last_seen, is_online)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                hostname = excluded.hostname,
                os = excluded.os,
                arch = excluded.arch,
                capabilities = excluded.capabilities,
                last_seen = excluded.last_seen,
                is_online = 1
        """,
            (worker_id, name, hostname, os, arch, caps_json, now, now),
        )
        await self._db.commit()

    async def set_worker_offline(self, worker_id: str):
        """Mark a worker as offline."""
        await self._db.execute(
            "UPDATE workers SET is_online = 0 WHERE id = ?", (worker_id,)
        )
        await self._db.commit()

    async def get_worker(self, worker_id: str) -> Optional[dict]:
        """Get a worker by ID."""
        async with self._db.execute(
            "SELECT * FROM workers WHERE id = ?", (worker_id,)
        ) as cursor:
            row = await cursor.fetchone()
            if row:
                return dict(row)
            return None

    async def get_all_workers(self) -> list[dict]:
        """Get all workers."""
        async with self._db.execute("SELECT * FROM workers ORDER BY last_seen DESC") as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    async def update_worker_name(self, worker_id: str, name: str):
        """Update a worker's display name."""
        await self._db.execute(
            "UPDATE workers SET name = ? WHERE id = ?", (name, worker_id)
        )
        await self._db.commit()

    async def delete_worker(self, worker_id: str):
        """Delete a worker."""
        await self._db.execute("DELETE FROM workers WHERE id = ?", (worker_id,))
        await self._db.commit()


    # OAuth client methods
    async def create_oauth_client(
        self,
        client_id: str,
        client_secret_hash: str,
        name: str,
        scopes: list[str],
        created_by: str,
    ):
        """Create a new OAuth client."""
        now = datetime.utcnow().isoformat()
        scopes_json = json.dumps(scopes)

        await self._db.execute(
            """
            INSERT INTO oauth_clients (client_id, client_secret_hash, name, scopes, created_at, created_by, is_active)
            VALUES (?, ?, ?, ?, ?, ?, 1)
            """,
            (client_id, client_secret_hash, name, scopes_json, now, created_by),
        )
        await self._db.commit()

    async def get_oauth_client(self, client_id: str) -> Optional[dict]:
        """Get an OAuth client by ID."""
        async with self._db.execute(
            "SELECT * FROM oauth_clients WHERE client_id = ? AND is_active = 1", (client_id,)
        ) as cursor:
            row = await cursor.fetchone()
            if row:
                result = dict(row)
                result["scopes"] = json.loads(result["scopes"])
                return result
            return None

    async def get_all_oauth_clients(self) -> list[dict]:
        """Get all active OAuth clients."""
        async with self._db.execute(
            "SELECT client_id, name, scopes, created_at, created_by, last_used FROM oauth_clients WHERE is_active = 1 ORDER BY created_at DESC"
        ) as cursor:
            rows = await cursor.fetchall()
            result = []
            for row in rows:
                item = dict(row)
                item["scopes"] = json.loads(item["scopes"])
                result.append(item)
            return result

    async def update_oauth_client_last_used(self, client_id: str):
        """Update last used timestamp for an OAuth client."""
        now = datetime.utcnow().isoformat()
        await self._db.execute(
            "UPDATE oauth_clients SET last_used = ? WHERE client_id = ?",
            (now, client_id),
        )
        await self._db.commit()

    async def revoke_oauth_client(self, client_id: str):
        """Revoke (deactivate) an OAuth client."""
        await self._db.execute(
            "UPDATE oauth_clients SET is_active = 0 WHERE client_id = ?",
            (client_id,),
        )
        await self._db.commit()

    async def delete_oauth_client(self, client_id: str):
        """Permanently delete an OAuth client."""
        await self._db.execute(
            "DELETE FROM oauth_clients WHERE client_id = ?",
            (client_id,),
        )
        await self._db.commit()

    # User management methods
    async def get_all_users(self) -> list[dict]:
        """Get all users."""
        async with self._db.execute("SELECT * FROM users") as cursor:
            rows = await cursor.fetchall()
            result = []
            for row in rows:
                item = dict(row)
                item["scopes"] = json.loads(item["scopes"])
                item["disabled"] = bool(item["disabled"])
                result.append(item)
            return result

    async def get_user(self, username: str) -> Optional[dict]:
        """Get a user by username."""
        async with self._db.execute(
            "SELECT * FROM users WHERE username = ?", (username,)
        ) as cursor:
            row = await cursor.fetchone()
            if row:
                result = dict(row)
                result["scopes"] = json.loads(result["scopes"])
                result["disabled"] = bool(result["disabled"])
                return result
            return None

    async def create_user(self, user: dict):
        """Create a new user."""
        now = datetime.utcnow().isoformat()
        scopes_json = json.dumps(user.get("scopes", ["read", "write"]))

        await self._db.execute(
            """
            INSERT INTO users (username, hashed_password, disabled, scopes, created_at)
            VALUES (?, ?, ?, ?, ?)
            """,
            (user["username"], user["hashed_password"], int(user.get("disabled", False)), scopes_json, now),
        )
        await self._db.commit()

    async def update_user_password(self, username: str, hashed_password: str):
        """Update a user's password."""
        await self._db.execute(
            "UPDATE users SET hashed_password = ? WHERE username = ?",
            (hashed_password, username),
        )
        await self._db.commit()

    async def update_username(self, old_username: str, new_username: str):
        """Update a user's username."""
        await self._db.execute(
            "UPDATE users SET username = ? WHERE username = ?",
            (new_username, old_username),
        )
        await self._db.commit()

    async def delete_user(self, username: str):
        """Delete a user."""
        await self._db.execute("DELETE FROM users WHERE username = ?", (username,))
        await self._db.commit()

    # Collection config methods
    async def get_collection_config(self, worker_id: str) -> Optional[dict]:
        """Get collection config for a worker."""
        async with self._db.execute(
            "SELECT * FROM collection_configs WHERE worker_id = ?", (worker_id,)
        ) as cursor:
            row = await cursor.fetchone()
            if row:
                result = dict(row)
                result["config"] = json.loads(result["config_json"])
                return result
            return None

    async def upsert_collection_config(
        self,
        worker_id: str,
        config: dict,
        encryption_public_key: Optional[str] = None,
        use_infrequent_access: bool = False,
    ):
        """Insert or update collection config for a worker."""
        now = datetime.utcnow().isoformat()
        config_json = json.dumps(config)

        await self._db.execute(
            """
            INSERT INTO collection_configs (worker_id, config_json, encryption_public_key, use_infrequent_access, updated_at)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(worker_id) DO UPDATE SET
                config_json = excluded.config_json,
                encryption_public_key = COALESCE(excluded.encryption_public_key, collection_configs.encryption_public_key),
                use_infrequent_access = excluded.use_infrequent_access,
                updated_at = excluded.updated_at
            """,
            (worker_id, config_json, encryption_public_key, int(use_infrequent_access), now),
        )
        await self._db.commit()

    async def set_encryption_key(self, worker_id: str, public_key: str):
        """Set encryption public key for a worker."""
        now = datetime.utcnow().isoformat()
        await self._db.execute(
            """
            UPDATE collection_configs SET encryption_public_key = ?, updated_at = ?
            WHERE worker_id = ?
            """,
            (public_key, now, worker_id),
        )
        await self._db.commit()

    # Collection session methods
    async def create_collection_session(self, session_id: str, worker_id: str) -> dict:
        """Create a new collection session."""
        now = datetime.utcnow().isoformat()
        await self._db.execute(
            """
            INSERT INTO collection_sessions (session_id, worker_id, started_at, status)
            VALUES (?, ?, ?, 'active')
            """,
            (session_id, worker_id, now),
        )
        await self._db.commit()
        return {"session_id": session_id, "worker_id": worker_id, "started_at": now, "status": "active"}

    async def end_collection_session(self, session_id: str, total_chunks: int = 0):
        """End a collection session."""
        now = datetime.utcnow().isoformat()
        await self._db.execute(
            """
            UPDATE collection_sessions SET ended_at = ?, status = 'completed', total_chunks = ?
            WHERE session_id = ?
            """,
            (now, total_chunks, session_id),
        )
        await self._db.commit()

    async def get_collection_session(self, session_id: str) -> Optional[dict]:
        """Get a collection session by ID."""
        async with self._db.execute(
            "SELECT * FROM collection_sessions WHERE session_id = ?", (session_id,)
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    async def get_worker_sessions(self, worker_id: str, limit: int = 50) -> list[dict]:
        """Get collection sessions for a worker."""
        async with self._db.execute(
            """
            SELECT * FROM collection_sessions
            WHERE worker_id = ?
            ORDER BY started_at DESC
            LIMIT ?
            """,
            (worker_id, limit),
        ) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    # Collection chunk methods
    async def create_collection_chunk(
        self,
        chunk_id: str,
        session_id: str,
        worker_id: str,
        chunk_index: int,
        local_path: str,
    ) -> dict:
        """Create a new chunk record."""
        now = datetime.utcnow().isoformat()
        start_timestamp = int(datetime.utcnow().timestamp() * 1000)

        await self._db.execute(
            """
            INSERT INTO collection_chunks
            (chunk_id, session_id, worker_id, chunk_index, start_timestamp, local_path, status)
            VALUES (?, ?, ?, ?, ?, ?, 'recording')
            """,
            (chunk_id, session_id, worker_id, chunk_index, start_timestamp, local_path),
        )
        await self._db.commit()
        return {
            "chunk_id": chunk_id,
            "session_id": session_id,
            "worker_id": worker_id,
            "chunk_index": chunk_index,
            "start_timestamp": start_timestamp,
            "local_path": local_path,
            "status": "recording",
        }

    async def complete_chunk(
        self,
        chunk_id: str,
        size_bytes: int,
        end_timestamp: Optional[int] = None,
    ):
        """Mark a chunk as complete."""
        if end_timestamp is None:
            end_timestamp = int(datetime.utcnow().timestamp() * 1000)

        await self._db.execute(
            """
            UPDATE collection_chunks
            SET end_timestamp = ?, size_bytes = ?, status = 'complete'
            WHERE chunk_id = ?
            """,
            (end_timestamp, size_bytes, chunk_id),
        )
        await self._db.commit()

    async def mark_chunk_uploaded(self, chunk_id: str, r2_path: str, encrypted: bool = False):
        """Mark a chunk as uploaded to R2."""
        await self._db.execute(
            """
            UPDATE collection_chunks
            SET r2_path = ?, encrypted = ?, status = 'uploaded'
            WHERE chunk_id = ?
            """,
            (r2_path, int(encrypted), chunk_id),
        )
        await self._db.commit()

    async def get_chunk(self, chunk_id: str) -> Optional[dict]:
        """Get a chunk by ID."""
        async with self._db.execute(
            "SELECT * FROM collection_chunks WHERE chunk_id = ?", (chunk_id,)
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    async def get_session_chunks(self, session_id: str) -> list[dict]:
        """Get all chunks for a session."""
        async with self._db.execute(
            """
            SELECT * FROM collection_chunks
            WHERE session_id = ?
            ORDER BY chunk_index ASC
            """,
            (session_id,),
        ) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    async def get_chunks_to_upload(self, limit: int = 10) -> list[dict]:
        """Get chunks that are complete but not yet uploaded."""
        async with self._db.execute(
            """
            SELECT * FROM collection_chunks
            WHERE status = 'complete' AND r2_path IS NULL
            ORDER BY start_timestamp ASC
            LIMIT ?
            """,
            (limit,),
        ) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    async def get_evictable_chunks(self, limit: int = 10) -> list[dict]:
        """Get chunks that have been uploaded and can be evicted from local cache."""
        async with self._db.execute(
            """
            SELECT * FROM collection_chunks
            WHERE status = 'uploaded' AND local_path IS NOT NULL
            ORDER BY start_timestamp ASC
            LIMIT ?
            """,
            (limit,),
        ) as cursor:
            rows = await cursor.fetchall()
            return [dict(row) for row in rows]

    async def clear_chunk_local_path(self, chunk_id: str):
        """Clear the local path for an evicted chunk."""
        await self._db.execute(
            "UPDATE collection_chunks SET local_path = NULL WHERE chunk_id = ?",
            (chunk_id,),
        )
        await self._db.commit()

    # R2 config methods
    async def get_r2_config(self) -> Optional[dict]:
        """Get R2 configuration."""
        async with self._db.execute("SELECT * FROM r2_config WHERE id = 1") as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    async def set_r2_config(
        self,
        endpoint: str,
        access_key_encrypted: str,
        secret_key_encrypted: str,
        bucket_normal: str,
        bucket_infrequent: Optional[str] = None,
    ):
        """Set R2 configuration."""
        await self._db.execute(
            """
            INSERT INTO r2_config (id, endpoint, access_key_encrypted, secret_key_encrypted, bucket_normal, bucket_infrequent)
            VALUES (1, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                endpoint = excluded.endpoint,
                access_key_encrypted = excluded.access_key_encrypted,
                secret_key_encrypted = excluded.secret_key_encrypted,
                bucket_normal = excluded.bucket_normal,
                bucket_infrequent = excluded.bucket_infrequent
            """,
            (endpoint, access_key_encrypted, secret_key_encrypted, bucket_normal, bucket_infrequent),
        )
        await self._db.commit()


# Global storage instance
storage = Storage()


# Convenience functions for module-level access
async def get_all_users() -> list[dict]:
    return await storage.get_all_users()


async def get_user(username: str) -> Optional[dict]:
    return await storage.get_user(username)


async def create_user(user: dict):
    return await storage.create_user(user)


async def update_user_password(username: str, hashed_password: str):
    return await storage.update_user_password(username, hashed_password)


async def update_username(old_username: str, new_username: str):
    return await storage.update_username(old_username, new_username)
