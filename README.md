# Corely - Remote Computer Management System

Corely is a remote computer management system that enables Claude Code (via MCP) to manage multiple remote machines. It provides shell execution, file operations, screen capture, input simulation, and interactive terminal sessions.

## Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Claude Code   │────▶│  Corely Server  │◀───▶│  Corely Worker  │
│    (via MCP)    │     │    (Python)     │     │     (Rust)      │
└─────────────────┘     └─────────────────┘     └─────────────────┘
        │                       │                       │
        │                       │                       ▼
        │                       │               ┌───────────────┐
        │                       │               │ Remote Machine│
        │                       │               └───────────────┘
        ▼                       ▼
┌─────────────────┐     ┌─────────────────┐
│  Python Client  │     │   Web Dashboard │
│    Library      │     │     (React)     │
└─────────────────┘     └─────────────────┘
```

## Components

### Worker (Rust)

The worker is a lightweight agent that runs on remote machines and connects to the server via WebSocket.

**Features:**
- Shell command execution with timeout
- File operations (read, write, edit, glob, grep)
- Screen capture
- Camera capture
- Keyboard/mouse input simulation
- Key recording
- Interactive PTY sessions (tmux-like)
- System information (CPU, memory, disk, GPU)
- Auto-install as system service
- Plugin support for custom tools

### Server (Python)

The server manages worker connections and exposes MCP tools for Claude Code integration.

**Features:**
- Worker connection management
- JWT authentication
- MCP server for Claude Code integration
- REST API for programmatic access
- WebSocket endpoints for streaming
- SQLite persistence

### Client Library (Python)

A Python library for programmatic access to workers.

**Features:**
- Sync and async APIs
- Full access to all worker capabilities
- Type-safe models with Pydantic

### Web Dashboard (React)

A retro-futuristic web interface for monitoring and controlling workers.

**Features:**
- Real-time worker status
- System monitoring
- Live screen viewing
- Terminal interface
- File browser

## Installation

### Prerequisites

- Rust 1.70+ (for worker)
- Python 3.10+ (for server and client)
- Node.js 18+ (for web dashboard)

### Worker

```bash
cd worker
cargo build --release
```

The binary will be at `target/release/corely-worker`.

### Server

```bash
cd server
pip install -e .
```

### Client Library

```bash
cd client
pip install -e .
```

### Web Dashboard

```bash
cd web
npm install
npm run build
```

## Usage

### Starting the Server

```bash
# HTTP mode (REST API)
corely-server --mode http --port 8000

# MCP mode (for Claude Code)
corely-server --mode mcp
```

### Starting a Worker

```bash
./corely-worker --server ws://localhost:8000/ws/worker --token corely-worker-secret
```

### Installing Worker as Service

```bash
./corely-worker --server ws://your-server.com/ws/worker --token your-token --install
```

This will:
- Request necessary permissions (screen recording, accessibility, camera, etc.)
- Install as a system service with auto-start
- Try multiple strategies: LaunchAgent/LaunchDaemon (macOS), systemd/cron (Linux), Registry/Task Scheduler (Windows)

### Using the Client Library

```python
from corely_client import CorelyClient

with CorelyClient("http://localhost:8000", "admin", "admin") as client:
    # List workers
    workers = client.list_workers()

    # Execute shell command
    result = client.bash("worker-id", "ls -la")
    print(result.stdout)

    # Read a file
    content = client.read("worker-id", "/etc/hosts")

    # Take a screenshot
    client.save_screenshot("worker-id", "screenshot.png")

    # Get system info
    info = client.system_info("worker-id")
    print(f"CPU: {info.cpu['usage_percent']}%")

    # Interactive session
    session = client.session_create("worker-id", name="my-session")
    session_id = session["session_id"]

    client.session_input("worker-id", session_id, "ls -la\n")
    time.sleep(0.5)
    output = client.session_read("worker-id", session_id)
    print(output["output"])

    client.session_kill("worker-id", session_id)
```

### Async Client

```python
import asyncio
from corely_client import AsyncCorelyClient

async def main():
    async with AsyncCorelyClient("http://localhost:8000", "admin", "admin") as client:
        workers = await client.list_workers()
        for worker in workers:
            if worker.is_online:
                info = await client.system_info(worker.id)
                print(f"{worker.name}: CPU {info.cpu['usage_percent']:.1f}%")

asyncio.run(main())
```

### Web Dashboard

Start the development server:

```bash
cd web
npm run dev
```

Then open http://localhost:3000 in your browser.

Default credentials: `admin` / `admin`

## MCP Integration

Add to your Claude Code MCP configuration:

```json
{
  "mcpServers": {
    "corely": {
      "command": "corely-server",
      "args": ["--mode", "mcp"]
    }
  }
}
```

### Available MCP Tools

For each connected worker:
- `{worker_id}_bash` - Execute shell commands
- `{worker_id}_read` - Read files
- `{worker_id}_write` - Write files
- `{worker_id}_edit` - Edit files
- `{worker_id}_glob` - Search for files
- `{worker_id}_grep` - Search file contents
- `{worker_id}_screenshot` - Capture screen
- `{worker_id}_system_info` - Get system info
- `{worker_id}_mouse_move` - Move mouse
- `{worker_id}_mouse_click` - Click mouse
- `{worker_id}_key_type` - Type text
- `{worker_id}_key_press` - Press keys

Global tools:
- `list_workers` - List all workers

## Configuration

### Environment Variables

**Server:**
- `CORELY_SECRET_KEY` - JWT secret key (auto-generated if not set)
- `CORELY_WORKER_TOKEN` - Pre-shared worker authentication token
- `CORELY_DB_PATH` - SQLite database path (default: `corely.db`)

**Worker:**
- `CORELY_SERVER` - Server WebSocket URL
- `CORELY_TOKEN` - Authentication token

### Worker CLI Options

```
Options:
  -s, --server <URL>     Server WebSocket URL
  -t, --token <TOKEN>    Authentication token
  -n, --name <NAME>      Worker name (defaults to hostname)
  -v, --verbose          Enable verbose logging
      --install          Install as system service
      --tools <PATH>     Path to plugins directory
```

## API Reference

### Authentication

```
POST /api/auth/login
Content-Type: application/x-www-form-urlencoded

username=admin&password=admin
```

Response:
```json
{
  "access_token": "eyJ...",
  "token_type": "bearer"
}
```

### Workers

```
GET /api/workers                    # List all workers
GET /api/workers/{id}               # Get worker details
PATCH /api/workers/{id}?name=...    # Update worker name
DELETE /api/workers/{id}            # Delete worker

POST /api/workers/{id}/call         # Call any worker method
POST /api/workers/{id}/shell        # Execute shell command
GET /api/workers/{id}/screen        # Capture screen
GET /api/workers/{id}/system        # Get system info
```

### WebSocket Endpoints

```
/ws/worker           # Worker connection endpoint
/ws/terminal/{id}    # Terminal session streaming
/ws/screen/{id}      # Screen capture streaming
```

## Security

- **Authentication**: JWT tokens for API access, pre-shared token for workers
- **Authorization**: Scope-based permissions (read, write, admin)
- **Transport**: TLS recommended for production
- **Permissions**: Workers request necessary OS permissions on install

## Development

### Running Tests

**Server:**
```bash
cd server
pip install -e ".[test]"
pytest tests/ -v
```

**Worker:**
```bash
cd worker
cargo test
```

**Integration tests:**
```bash
RUN_INTEGRATION_TESTS=1 pytest tests/test_integration.py -v
```

### Building for Production

**Worker:**
```bash
cd worker
cargo build --release
```

**Server:**
```bash
cd server
pip install build
python -m build
```

**Web Dashboard:**
```bash
cd web
npm run build
```

## License

MIT License

## Contributing

Contributions are welcome! Please read our contributing guidelines before submitting PRs.
