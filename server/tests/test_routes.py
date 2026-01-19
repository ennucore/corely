"""Tests for HTTP routes."""

import pytest
from unittest.mock import AsyncMock, patch, MagicMock
from fastapi.testclient import TestClient
from fastapi import FastAPI

from corely_server.routes import router
from corely_server.auth import create_access_token


@pytest.fixture
def app():
    """Create a test FastAPI app."""
    app = FastAPI()
    app.include_router(router, prefix="/api")
    return app


@pytest.fixture
def client(app):
    """Create a test client."""
    return TestClient(app)


@pytest.fixture
def auth_token():
    """Create a valid auth token."""
    return create_access_token({"sub": "admin", "scopes": ["read", "write", "admin"]})


@pytest.fixture
def auth_headers(auth_token):
    """Create auth headers."""
    return {"Authorization": f"Bearer {auth_token}"}


class TestAuthRoutes:
    """Tests for authentication routes."""

    def test_login_success(self, client):
        """Test successful login."""
        response = client.post(
            "/api/auth/login",
            data={"username": "admin", "password": "admin"},
        )
        assert response.status_code == 200
        data = response.json()
        assert "access_token" in data
        assert data["token_type"] == "bearer"

    def test_login_invalid_credentials(self, client):
        """Test login with invalid credentials."""
        response = client.post(
            "/api/auth/login",
            data={"username": "admin", "password": "wrong"},
        )
        assert response.status_code == 401

    def test_get_me_authenticated(self, client, auth_headers):
        """Test getting current user info."""
        response = client.get("/api/auth/me", headers=auth_headers)
        assert response.status_code == 200
        data = response.json()
        assert data["username"] == "admin"

    def test_get_me_unauthenticated(self, client):
        """Test getting current user without auth."""
        response = client.get("/api/auth/me")
        assert response.status_code == 401  # Unauthorized - no auth header


class TestWorkerRoutes:
    """Tests for worker management routes."""

    def test_list_workers(self, client, auth_headers):
        """Test listing workers."""
        with patch("corely_server.routes.storage") as mock_storage, \
             patch("corely_server.routes.worker_manager") as mock_manager:
            mock_storage.get_all_workers = AsyncMock(return_value=[
                {
                    "id": "worker-1",
                    "name": "Worker 1",
                    "hostname": "host1",
                    "os": "Linux",
                    "arch": "x86_64",
                    "capabilities": '["shell.exec"]',
                    "is_online": 1,
                    "last_seen": "2024-01-01T00:00:00",
                }
            ])
            mock_manager.get_all_workers = AsyncMock(return_value=[
                MagicMock(id="worker-1")
            ])

            response = client.get("/api/workers", headers=auth_headers)
            assert response.status_code == 200
            data = response.json()
            assert "workers" in data
            assert len(data["workers"]) == 1

    def test_get_worker(self, client, auth_headers):
        """Test getting a specific worker."""
        with patch("corely_server.routes.storage") as mock_storage, \
             patch("corely_server.routes.worker_manager") as mock_manager:
            mock_storage.get_worker = AsyncMock(return_value={
                "id": "worker-1",
                "name": "Worker 1",
                "hostname": "host1",
                "os": "Linux",
                "arch": "x86_64",
                "capabilities": '["shell.exec"]',
                "is_online": 1,
                "last_seen": "2024-01-01T00:00:00",
            })
            mock_manager.get_all_workers = AsyncMock(return_value=[])

            response = client.get("/api/workers/worker-1", headers=auth_headers)
            assert response.status_code == 200
            data = response.json()
            assert data["id"] == "worker-1"

    def test_get_worker_not_found(self, client, auth_headers):
        """Test getting a worker that doesn't exist."""
        with patch("corely_server.routes.storage") as mock_storage:
            mock_storage.get_worker = AsyncMock(return_value=None)

            response = client.get("/api/workers/nonexistent", headers=auth_headers)
            assert response.status_code == 404

    def test_update_worker(self, client, auth_headers):
        """Test updating a worker."""
        with patch("corely_server.routes.storage") as mock_storage:
            mock_storage.get_worker = AsyncMock(return_value={"id": "worker-1"})
            mock_storage.update_worker_name = AsyncMock()

            response = client.patch(
                "/api/workers/worker-1?name=NewName",
                headers=auth_headers,
            )
            assert response.status_code == 200
            mock_storage.update_worker_name.assert_called_once()

    def test_delete_worker(self, client, auth_headers):
        """Test deleting a worker."""
        with patch("corely_server.routes.storage") as mock_storage:
            mock_storage.delete_worker = AsyncMock()

            response = client.delete("/api/workers/worker-1", headers=auth_headers)
            assert response.status_code == 200
            mock_storage.delete_worker.assert_called_once_with("worker-1")


class TestWorkerCommandRoutes:
    """Tests for worker command routes."""

    def test_call_worker_not_connected(self, client, auth_headers):
        """Test calling a method on disconnected worker."""
        with patch("corely_server.routes.worker_manager") as mock_manager:
            mock_manager.get_worker = AsyncMock(return_value=None)

            response = client.post(
                "/api/workers/worker-1/call",
                json={"method": "shell.exec", "params": {"command": "ls"}},
                headers=auth_headers,
            )
            assert response.status_code == 404

    def test_shell_command_not_connected(self, client, auth_headers):
        """Test shell command on disconnected worker."""
        with patch("corely_server.routes.worker_manager") as mock_manager:
            mock_manager.get_worker = AsyncMock(return_value=None)

            response = client.post(
                "/api/workers/worker-1/shell?command=ls",
                headers=auth_headers,
            )
            assert response.status_code == 404

    def test_screen_capture_not_connected(self, client, auth_headers):
        """Test screen capture on disconnected worker."""
        with patch("corely_server.routes.worker_manager") as mock_manager:
            mock_manager.get_worker = AsyncMock(return_value=None)

            response = client.get(
                "/api/workers/worker-1/screen",
                headers=auth_headers,
            )
            assert response.status_code == 404

    def test_system_info_not_connected(self, client, auth_headers):
        """Test system info on disconnected worker."""
        with patch("corely_server.routes.worker_manager") as mock_manager:
            mock_manager.get_worker = AsyncMock(return_value=None)

            response = client.get(
                "/api/workers/worker-1/system",
                headers=auth_headers,
            )
            assert response.status_code == 404
