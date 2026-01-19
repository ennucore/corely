"""Integration tests for Corely server and workers.

These tests require a running server and worker to execute.
Run with: pytest tests/test_integration.py -v --run-integration
"""

import asyncio
import pytest
import subprocess
import time
import os
from pathlib import Path

# Skip all tests in this file unless --run-integration is passed
pytestmark = pytest.mark.skipif(
    not os.environ.get("RUN_INTEGRATION_TESTS"),
    reason="Integration tests require RUN_INTEGRATION_TESTS=1"
)


class TestServerWorkerIntegration:
    """Integration tests for server-worker communication."""

    @pytest.fixture(scope="class")
    def server_process(self):
        """Start the server for testing."""
        server_dir = Path(__file__).parent.parent
        env = os.environ.copy()
        env["PYTHONPATH"] = str(server_dir)

        process = subprocess.Popen(
            ["python", "-m", "corely_server.main", "--port", "8765"],
            cwd=server_dir,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

        # Wait for server to start
        time.sleep(2)

        yield process

        process.terminate()
        process.wait()

    @pytest.mark.asyncio
    async def test_server_health(self, server_process):
        """Test that the server is running and healthy."""
        import httpx

        async with httpx.AsyncClient() as client:
            # Login
            response = await client.post(
                "http://localhost:8765/api/auth/login",
                data={"username": "admin", "password": "admin"},
            )
            assert response.status_code == 200
            token = response.json()["access_token"]

            # List workers (should be empty initially)
            response = await client.get(
                "http://localhost:8765/api/workers",
                headers={"Authorization": f"Bearer {token}"},
            )
            assert response.status_code == 200
            data = response.json()
            assert "workers" in data

    @pytest.mark.asyncio
    async def test_auth_flow(self, server_process):
        """Test the authentication flow."""
        import httpx

        async with httpx.AsyncClient() as client:
            # Test invalid credentials
            response = await client.post(
                "http://localhost:8765/api/auth/login",
                data={"username": "admin", "password": "wrong"},
            )
            assert response.status_code == 401

            # Test valid credentials
            response = await client.post(
                "http://localhost:8765/api/auth/login",
                data={"username": "admin", "password": "admin"},
            )
            assert response.status_code == 200
            assert "access_token" in response.json()

            # Test /me endpoint
            token = response.json()["access_token"]
            response = await client.get(
                "http://localhost:8765/api/auth/me",
                headers={"Authorization": f"Bearer {token}"},
            )
            assert response.status_code == 200
            assert response.json()["username"] == "admin"


class TestClientLibraryIntegration:
    """Integration tests for the client library."""

    @pytest.fixture(scope="class")
    def server_process(self):
        """Start the server for testing."""
        server_dir = Path(__file__).parent.parent
        env = os.environ.copy()
        env["PYTHONPATH"] = str(server_dir)

        process = subprocess.Popen(
            ["python", "-m", "corely_server.main", "--port", "8766"],
            cwd=server_dir,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

        time.sleep(2)
        yield process
        process.terminate()
        process.wait()

    def test_client_login(self, server_process):
        """Test client login."""
        import sys
        sys.path.insert(0, str(Path(__file__).parent.parent.parent / "client"))

        from corely_client import CorelyClient

        with CorelyClient("http://localhost:8766", "admin", "admin") as client:
            workers = client.list_workers()
            assert isinstance(workers, list)

    @pytest.mark.asyncio
    async def test_async_client_login(self, server_process):
        """Test async client login."""
        import sys
        sys.path.insert(0, str(Path(__file__).parent.parent.parent / "client"))

        from corely_client import AsyncCorelyClient

        async with AsyncCorelyClient("http://localhost:8766", "admin", "admin") as client:
            workers = await client.list_workers()
            assert isinstance(workers, list)


class TestEndToEndWorkflow:
    """End-to-end workflow tests."""

    @pytest.mark.asyncio
    async def test_full_workflow_mock(self):
        """Test a full workflow with mocked components.

        This test simulates the full workflow without requiring
        actual server/worker processes.
        """
        from unittest.mock import AsyncMock, patch, MagicMock
        from corely_server.worker_manager import WorkerManager, WorkerConnection, WorkerInfo

        # Create a mock worker connection
        manager = WorkerManager()
        mock_ws = AsyncMock()
        mock_ws.send_text = AsyncMock()

        connection = await manager.register_worker(mock_ws)
        connection.info = WorkerInfo(
            id=connection.worker_id,
            name="Test Worker",
            hostname="test.local",
            os="Linux",
            arch="x86_64",
            capabilities=["shell.exec", "fs.read"],
        )

        # Verify worker is registered
        workers = await manager.get_all_workers()
        assert len(workers) == 1
        assert workers[0].name == "Test Worker"

        # Simulate sending a message
        await manager.send_message(connection, {"method": "shell.exec", "params": {}})
        mock_ws.send_text.assert_called_once()

        # Cleanup
        with patch("corely_server.worker_manager.storage") as mock_storage:
            mock_storage.set_worker_offline = AsyncMock()
            await manager.unregister_worker(connection.worker_id)

        workers = await manager.get_all_workers()
        assert len(workers) == 0
