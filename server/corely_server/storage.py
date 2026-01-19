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
