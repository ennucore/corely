"""Tests for Corely client library."""

import pytest
from unittest.mock import AsyncMock, MagicMock, patch

from corely_client.client import (
    Worker,
    ShellResult,
    FileContent,
    GlobResult,
    GrepMatch,
    GrepFileResult,
    GrepResult,
    ScreenCapture,
    SystemInfo,
    AsyncCorelyClient,
    CorelyClient,
)


class TestModels:
    """Tests for Pydantic models."""

    def test_worker_model(self):
        """Test Worker model creation."""
        worker = Worker(
            id="worker-1",
            name="Test Worker",
            hostname="test.local",
            os="Linux",
            arch="x86_64",
            capabilities=["shell.exec", "fs.read"],
            is_online=True,
        )
        assert worker.id == "worker-1"
        assert worker.name == "Test Worker"
        assert worker.is_online is True
        assert len(worker.capabilities) == 2

    def test_worker_model_defaults(self):
        """Test Worker model with minimal fields."""
        worker = Worker(id="worker-2", name="Minimal Worker")
        assert worker.id == "worker-2"
        assert worker.hostname is None
        assert worker.is_online is False
        assert worker.capabilities == []

    def test_shell_result_model(self):
        """Test ShellResult model."""
        result = ShellResult(
            stdout="hello world",
            stderr="",
            exit_code=0,
            timed_out=False,
        )
        assert result.stdout == "hello world"
        assert result.exit_code == 0
        assert result.timed_out is False

    def test_shell_result_with_error(self):
        """Test ShellResult with error output."""
        result = ShellResult(
            stdout="",
            stderr="command not found",
            exit_code=127,
        )
        assert result.exit_code == 127
        assert result.stderr == "command not found"

    def test_file_content_model(self):
        """Test FileContent model."""
        content = FileContent(
            content="line 1\nline 2\nline 3",
            total_lines=100,
            offset=0,
            lines_returned=3,
        )
        assert "line 1" in content.content
        assert content.total_lines == 100
        assert content.lines_returned == 3

    def test_glob_result_model(self):
        """Test GlobResult model."""
        result = GlobResult(
            matches=["file1.py", "file2.py", "file3.py"],
            count=3,
        )
        assert len(result.matches) == 3
        assert result.count == 3

    def test_grep_result_model(self):
        """Test GrepResult model."""
        result = GrepResult(
            results=[
                GrepFileResult(
                    file="test.py",
                    matches=[
                        GrepMatch(line=10, content="def test_function():"),
                        GrepMatch(line=20, content="def another_test():"),
                    ],
                )
            ],
            files_matched=1,
        )
        assert result.files_matched == 1
        assert len(result.results[0].matches) == 2
        assert result.results[0].matches[0].line == 10

    def test_screen_capture_model(self):
        """Test ScreenCapture model."""
        capture = ScreenCapture(
            width=1920,
            height=1080,
            format="png",
            data="base64encodeddata==",
        )
        assert capture.width == 1920
        assert capture.height == 1080
        assert capture.format == "png"

    def test_system_info_model(self):
        """Test SystemInfo model."""
        info = SystemInfo(
            hostname="test-machine",
            os={"name": "Linux", "version": "5.10"},
            cpu={"cores": 8, "usage_percent": 25.5},
            memory={"total": 16000000000, "used": 8000000000},
            swap={"total": 8000000000, "used": 1000000000},
            disks=[{"name": "/", "total_space": 500000000000}],
            network=[{"name": "eth0", "received": 1000000}],
            uptime=86400,
        )
        assert info.hostname == "test-machine"
        assert info.cpu["cores"] == 8
        assert info.uptime == 86400


class TestAsyncCorelyClient:
    """Tests for AsyncCorelyClient."""

    def test_client_initialization(self):
        """Test client initialization."""
        client = AsyncCorelyClient(
            base_url="http://localhost:8000",
            username="admin",
            password="admin",
        )
        assert client.base_url == "http://localhost:8000"
        assert client._username == "admin"
        assert client._token is None

    def test_client_with_token(self):
        """Test client initialization with token."""
        client = AsyncCorelyClient(
            base_url="http://localhost:8000",
            token="existing-token",
        )
        assert client._token == "existing-token"

    def test_client_strips_trailing_slash(self):
        """Test that base_url trailing slash is stripped."""
        client = AsyncCorelyClient(base_url="http://localhost:8000/")
        assert client.base_url == "http://localhost:8000"

    def test_headers_without_token(self):
        """Test headers when no token is set."""
        client = AsyncCorelyClient(base_url="http://localhost:8000")
        headers = client._headers()
        assert "Authorization" not in headers

    def test_headers_with_token(self):
        """Test headers when token is set."""
        client = AsyncCorelyClient(
            base_url="http://localhost:8000", token="test-token"
        )
        headers = client._headers()
        assert headers["Authorization"] == "Bearer test-token"


class TestCorelyClient:
    """Tests for synchronous CorelyClient."""

    def test_sync_client_initialization(self):
        """Test sync client initialization."""
        client = CorelyClient(
            base_url="http://localhost:8000",
            username="admin",
            password="admin",
        )
        assert client._async_client.base_url == "http://localhost:8000"

    def test_sync_client_has_same_methods(self):
        """Test that sync client has the same methods as async client."""
        async_methods = [
            m for m in dir(AsyncCorelyClient)
            if not m.startswith("_") and callable(getattr(AsyncCorelyClient, m))
        ]
        sync_methods = [
            m for m in dir(CorelyClient)
            if not m.startswith("_") and callable(getattr(CorelyClient, m))
        ]

        # All async methods (except context manager methods) should exist in sync
        for method in async_methods:
            if method not in ("__aenter__", "__aexit__"):
                assert method in sync_methods, f"Method {method} missing from CorelyClient"


class TestClientMethods:
    """Tests for client method behavior with mocked responses."""

    @pytest.mark.asyncio
    async def test_login_sets_token(self):
        """Test that login sets the token."""
        client = AsyncCorelyClient(base_url="http://localhost:8000")

        with patch.object(client._client, "post", new_callable=AsyncMock) as mock_post:
            mock_response = MagicMock()
            mock_response.json.return_value = {"access_token": "new-token"}
            mock_response.raise_for_status = MagicMock()
            mock_post.return_value = mock_response

            token = await client.login("admin", "admin")

            assert token == "new-token"
            assert client._token == "new-token"
            mock_post.assert_called_once()

    @pytest.mark.asyncio
    async def test_list_workers(self):
        """Test list_workers method."""
        client = AsyncCorelyClient(
            base_url="http://localhost:8000", token="test-token"
        )

        with patch.object(client._client, "get", new_callable=AsyncMock) as mock_get:
            mock_response = MagicMock()
            mock_response.json.return_value = {
                "workers": [
                    {"id": "w1", "name": "Worker 1", "is_online": True},
                    {"id": "w2", "name": "Worker 2", "is_online": False},
                ]
            }
            mock_response.raise_for_status = MagicMock()
            mock_get.return_value = mock_response

            workers = await client.list_workers()

            assert len(workers) == 2
            assert workers[0].id == "w1"
            assert workers[1].is_online is False

    @pytest.mark.asyncio
    async def test_call_method(self):
        """Test generic call method."""
        client = AsyncCorelyClient(
            base_url="http://localhost:8000", token="test-token"
        )

        with patch.object(client._client, "post", new_callable=AsyncMock) as mock_post:
            mock_response = MagicMock()
            mock_response.json.return_value = {"result": {"status": "ok"}}
            mock_response.raise_for_status = MagicMock()
            mock_post.return_value = mock_response

            result = await client.call("worker-1", "test.method", {"param": "value"})

            assert result == {"status": "ok"}
            mock_post.assert_called_once()
            call_args = mock_post.call_args
            assert "worker-1" in call_args[0][0]

    @pytest.mark.asyncio
    async def test_bash_command(self):
        """Test bash command execution."""
        client = AsyncCorelyClient(
            base_url="http://localhost:8000", token="test-token"
        )

        with patch.object(client._client, "post", new_callable=AsyncMock) as mock_post:
            mock_response = MagicMock()
            mock_response.json.return_value = {
                "result": {
                    "stdout": "hello",
                    "stderr": "",
                    "exit_code": 0,
                    "timed_out": False,
                }
            }
            mock_response.raise_for_status = MagicMock()
            mock_post.return_value = mock_response

            result = await client.bash("worker-1", "echo hello")

            assert isinstance(result, ShellResult)
            assert result.stdout == "hello"
            assert result.exit_code == 0


@pytest.fixture
def conftest_setup():
    """Configure pytest-asyncio."""
    pass
