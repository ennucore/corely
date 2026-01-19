"""Tests for worker manager module."""

import pytest
import asyncio
from unittest.mock import AsyncMock, MagicMock, patch

from corely_server.worker_manager import WorkerManager, WorkerConnection, WorkerInfo


@pytest.fixture
def worker_manager():
    """Create a fresh WorkerManager instance."""
    return WorkerManager()


@pytest.fixture
def mock_websocket():
    """Create a mock WebSocket."""
    ws = AsyncMock()
    ws.send_text = AsyncMock()
    return ws


class TestWorkerManager:
    """Tests for WorkerManager class."""

    @pytest.mark.asyncio
    async def test_register_worker(self, worker_manager, mock_websocket):
        """Test registering a new worker."""
        connection = await worker_manager.register_worker(mock_websocket)

        assert connection.worker_id is not None
        assert len(connection.worker_id) == 36  # UUID format
        assert connection.websocket == mock_websocket

    @pytest.mark.asyncio
    async def test_get_worker(self, worker_manager, mock_websocket):
        """Test getting a registered worker."""
        connection = await worker_manager.register_worker(mock_websocket)

        retrieved = await worker_manager.get_worker(connection.worker_id)
        assert retrieved == connection

    @pytest.mark.asyncio
    async def test_get_nonexistent_worker(self, worker_manager):
        """Test getting a worker that doesn't exist."""
        result = await worker_manager.get_worker("nonexistent-id")
        assert result is None

    @pytest.mark.asyncio
    async def test_unregister_worker(self, worker_manager, mock_websocket):
        """Test unregistering a worker."""
        connection = await worker_manager.register_worker(mock_websocket)
        worker_id = connection.worker_id

        with patch("corely_server.worker_manager.storage") as mock_storage:
            mock_storage.set_worker_offline = AsyncMock()
            await worker_manager.unregister_worker(worker_id)
            mock_storage.set_worker_offline.assert_called_once_with(worker_id)

        result = await worker_manager.get_worker(worker_id)
        assert result is None

    @pytest.mark.asyncio
    async def test_get_all_workers(self, worker_manager, mock_websocket):
        """Test getting all connected workers."""
        # Register multiple workers
        conn1 = await worker_manager.register_worker(mock_websocket)
        conn2 = await worker_manager.register_worker(AsyncMock())

        # Add info to workers
        conn1.info = WorkerInfo(id=conn1.worker_id, name="Worker 1")
        conn2.info = WorkerInfo(id=conn2.worker_id, name="Worker 2")

        workers = await worker_manager.get_all_workers()
        assert len(workers) == 2

    @pytest.mark.asyncio
    async def test_send_message(self, worker_manager, mock_websocket):
        """Test sending a message to a worker."""
        connection = await worker_manager.register_worker(mock_websocket)

        message = {"jsonrpc": "2.0", "method": "test"}
        await worker_manager.send_message(connection, message)

        mock_websocket.send_text.assert_called_once()
        sent_data = mock_websocket.send_text.call_args[0][0]
        assert "test" in sent_data

    @pytest.mark.asyncio
    async def test_handle_hello(self, worker_manager, mock_websocket):
        """Test handling worker hello message."""
        connection = await worker_manager.register_worker(mock_websocket)

        with patch("corely_server.worker_manager.storage") as mock_storage:
            mock_storage.upsert_worker = AsyncMock()

            await worker_manager.handle_hello(
                connection,
                {
                    "name": "Test Worker",
                    "hostname": "test.local",
                    "os": "Linux",
                    "arch": "x86_64",
                    "capabilities": ["shell.exec"],
                },
            )

            assert connection.info is not None
            assert connection.info.name == "Test Worker"
            assert connection.info.hostname == "test.local"
            mock_storage.upsert_worker.assert_called_once()

    @pytest.mark.asyncio
    async def test_handle_response(self, worker_manager, mock_websocket):
        """Test handling a response from a worker."""
        connection = await worker_manager.register_worker(mock_websocket)

        # Create a pending request
        future = asyncio.get_event_loop().create_future()
        connection.pending_requests["test-request-id"] = future

        # Handle response
        await worker_manager.handle_response(
            connection,
            {"id": "test-request-id", "result": {"status": "ok"}},
        )

        assert future.done()
        assert future.result() == {"status": "ok"}

    @pytest.mark.asyncio
    async def test_handle_error_response(self, worker_manager, mock_websocket):
        """Test handling an error response from a worker."""
        connection = await worker_manager.register_worker(mock_websocket)

        future = asyncio.get_event_loop().create_future()
        connection.pending_requests["test-request-id"] = future

        await worker_manager.handle_response(
            connection,
            {
                "id": "test-request-id",
                "error": {"code": -32000, "message": "Test error"},
            },
        )

        assert future.done()
        with pytest.raises(Exception) as exc_info:
            future.result()
        assert "Test error" in str(exc_info.value)


class TestWorkerInfo:
    """Tests for WorkerInfo model."""

    def test_create_worker_info(self):
        """Test creating WorkerInfo."""
        info = WorkerInfo(
            id="test-id",
            name="Test Worker",
            hostname="test.local",
            os="Linux",
            arch="x86_64",
            capabilities=["shell.exec", "fs.read"],
        )

        assert info.id == "test-id"
        assert info.name == "Test Worker"
        assert info.hostname == "test.local"
        assert info.os == "Linux"
        assert info.arch == "x86_64"
        assert "shell.exec" in info.capabilities

    def test_worker_info_defaults(self):
        """Test WorkerInfo default values."""
        info = WorkerInfo(id="test-id", name="Test")

        assert info.hostname is None
        assert info.os is None
        assert info.capabilities == []
