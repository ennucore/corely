"""Tests for storage module."""

import pytest
import pytest_asyncio
import tempfile
import os

from corely_server.storage import Storage


@pytest.fixture
def temp_db():
    """Create a temporary database file."""
    fd, path = tempfile.mkstemp(suffix=".db")
    os.close(fd)
    yield path
    os.unlink(path)


@pytest_asyncio.fixture
async def storage(temp_db):
    """Create and initialize a storage instance."""
    s = Storage(temp_db)
    await s.init()
    yield s
    await s.close()


class TestStorage:
    """Tests for Storage class."""

    @pytest.mark.asyncio
    async def test_init_creates_tables(self, storage):
        """Test that init creates required tables."""
        # If we got here without error, tables were created
        assert storage._db is not None

    @pytest.mark.asyncio
    async def test_upsert_worker(self, storage):
        """Test inserting a new worker."""
        await storage.upsert_worker(
            worker_id="test-worker-1",
            name="Test Worker",
            hostname="test.local",
            os="Linux",
            arch="x86_64",
            capabilities=["shell.exec", "fs.read"],
        )

        worker = await storage.get_worker("test-worker-1")
        assert worker is not None
        assert worker["name"] == "Test Worker"
        assert worker["hostname"] == "test.local"
        assert worker["os"] == "Linux"
        assert worker["arch"] == "x86_64"

    @pytest.mark.asyncio
    async def test_upsert_worker_updates_existing(self, storage):
        """Test that upsert updates existing worker."""
        await storage.upsert_worker(
            worker_id="test-worker-2",
            name="Original Name",
            hostname="original.local",
        )

        await storage.upsert_worker(
            worker_id="test-worker-2",
            name="Updated Name",
            hostname="updated.local",
        )

        worker = await storage.get_worker("test-worker-2")
        assert worker["name"] == "Updated Name"
        assert worker["hostname"] == "updated.local"

    @pytest.mark.asyncio
    async def test_get_nonexistent_worker(self, storage):
        """Test getting a worker that doesn't exist."""
        worker = await storage.get_worker("nonexistent-worker")
        assert worker is None

    @pytest.mark.asyncio
    async def test_set_worker_offline(self, storage):
        """Test marking a worker as offline."""
        await storage.upsert_worker(
            worker_id="test-worker-3",
            name="Online Worker",
        )

        await storage.set_worker_offline("test-worker-3")

        worker = await storage.get_worker("test-worker-3")
        assert worker["is_online"] == 0

    @pytest.mark.asyncio
    async def test_get_all_workers(self, storage):
        """Test getting all workers."""
        await storage.upsert_worker(worker_id="worker-a", name="Worker A")
        await storage.upsert_worker(worker_id="worker-b", name="Worker B")
        await storage.upsert_worker(worker_id="worker-c", name="Worker C")

        workers = await storage.get_all_workers()
        assert len(workers) == 3

        names = {w["name"] for w in workers}
        assert names == {"Worker A", "Worker B", "Worker C"}

    @pytest.mark.asyncio
    async def test_update_worker_name(self, storage):
        """Test updating a worker's display name."""
        await storage.upsert_worker(worker_id="worker-d", name="Old Name")

        await storage.update_worker_name("worker-d", "New Name")

        worker = await storage.get_worker("worker-d")
        assert worker["name"] == "New Name"

    @pytest.mark.asyncio
    async def test_delete_worker(self, storage):
        """Test deleting a worker."""
        await storage.upsert_worker(worker_id="worker-e", name="To Delete")

        await storage.delete_worker("worker-e")

        worker = await storage.get_worker("worker-e")
        assert worker is None
