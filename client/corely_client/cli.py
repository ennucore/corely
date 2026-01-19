#!/usr/bin/env python3
"""
Corely CLI - Remote machine management tool.

Usage:
    corely login                     Login to server with 2FA
    corely server start              Start the server in background
    corely server stop               Stop the background server
    corely ls                        List all machines
    corely ssh <machine>             Interactive shell session
    corely tmux <machine> [session]  Tmux-like session management
    corely code <machine> [path]     Claude Code-like session
    corely cp <src> <dst>            SCP-like file transfer
    corely rm <machine>              Uninstall worker from machine
    corely exec <machine> <cmd>      Execute a single command
"""

import argparse
import asyncio
import getpass
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Optional

import httpx

from .client import AsyncCorelyClient, CorelyClient

# Configuration
CONFIG_DIR = Path.home() / ".config" / "corely"
CONFIG_FILE = CONFIG_DIR / "config.json"
SERVER_PID_FILE = CONFIG_DIR / "server.pid"
SERVER_LOG_FILE = CONFIG_DIR / "server.log"

DEFAULT_SERVER_URL = os.environ.get("CORELY_SERVER", "http://127.0.0.1:8000")

_server_url = DEFAULT_SERVER_URL


def load_config() -> dict:
    """Load configuration from file."""
    if CONFIG_FILE.exists():
        try:
            return json.loads(CONFIG_FILE.read_text())
        except:
            pass
    return {}


def save_config(config: dict):
    """Save configuration to file."""
    CONFIG_DIR.mkdir(parents=True, exist_ok=True)
    CONFIG_FILE.write_text(json.dumps(config, indent=2))
    # Protect the config file
    CONFIG_FILE.chmod(0o600)


def get_client() -> CorelyClient:
    """Create and authenticate a client using saved token."""
    config = load_config()

    # Check for saved token for this server
    tokens = config.get("tokens", {})
    token = tokens.get(_server_url)

    if not token:
        print(f"Not logged in to {_server_url}")
        print("Run: corely login")
        sys.exit(1)

    client = CorelyClient(_server_url, token=token)
    return client


def resolve_worker(client: CorelyClient, name_or_id: str) -> str:
    """Resolve a worker name or partial ID to full worker ID."""
    workers = client.list_workers()

    # Try exact ID match
    for w in workers:
        if w.id == name_or_id:
            return w.id

    # Try name match
    for w in workers:
        if w.name == name_or_id:
            return w.id

    # Try partial ID match
    for w in workers:
        if w.id.startswith(name_or_id):
            return w.id

    # Try partial name match (case insensitive)
    for w in workers:
        if name_or_id.lower() in w.name.lower():
            return w.id

    raise ValueError(f"Worker not found: {name_or_id}")


# =============================================================================
# Server Management
# =============================================================================

def cmd_server_start(args):
    """Start the Corely server in background."""
    SERVER_PID_FILE.parent.mkdir(parents=True, exist_ok=True)
    SERVER_LOG_FILE.parent.mkdir(parents=True, exist_ok=True)

    # Check if already running
    if SERVER_PID_FILE.exists():
        pid = int(SERVER_PID_FILE.read_text().strip())
        try:
            os.kill(pid, 0)
            print(f"Server already running (PID {pid})")
            return
        except OSError:
            SERVER_PID_FILE.unlink()

    # Find server path - try multiple locations
    server_path = None
    server_cmd = None

    # Option 1: Try importing the installed module
    try:
        import corely_server
        server_path = Path(corely_server.__file__).parent.parent
        server_cmd = [sys.executable, "-m", "corely_server"]
    except ImportError:
        pass

    # Option 2: Try relative path from this file (sibling directory)
    # cli.py is at client/corely_client/cli.py, server is at server/
    if server_path is None:
        candidate = Path(__file__).parent.parent.parent / "server"
        if (candidate / "corely_server").exists():
            server_path = candidate
            # Use uv run if available (for development), fall back to python -m
            uv_path = shutil.which("uv")
            if uv_path:
                server_cmd = [uv_path, "run", "python", "-m", "corely_server"]
            else:
                server_cmd = [sys.executable, "-m", "corely_server"]

    # Option 3: Check CORELY_SERVER_PATH env var
    if server_path is None:
        env_path = os.environ.get("CORELY_SERVER_PATH")
        if env_path and Path(env_path).exists():
            server_path = Path(env_path)
            server_cmd = [sys.executable, "-m", "corely_server"]

    if server_path is None or server_cmd is None:
        print("Error: corely_server not found.")
        print("Options:")
        print("  1. Install corely_server: cd server && pip install -e .")
        print("  2. Set CORELY_SERVER_PATH=/path/to/server")
        sys.exit(1)

    # Start server
    print("Starting Corely server...")
    print(f"  Server path: {server_path}")

    env = os.environ.copy()
    env["PYTHONPATH"] = str(server_path)

    with open(SERVER_LOG_FILE, "w") as log:
        proc = subprocess.Popen(
            server_cmd + ["--host", "0.0.0.0", "--port", "8000"],
            cwd=str(server_path),
            stdout=log,
            stderr=log,
            env=env,
            start_new_session=True,
        )

    SERVER_PID_FILE.write_text(str(proc.pid))

    # Wait for server to be ready
    print("Waiting for server to start...")
    for i in range(30):
        time.sleep(0.5)
        try:
            import httpx
            resp = httpx.get(f"{_server_url}/health", timeout=1)
            if resp.status_code == 200:
                print(f"Server started (PID {proc.pid})")
                print(f"  URL: {_server_url}")
                print(f"  Log: {SERVER_LOG_FILE}")
                return
        except:
            pass

    print(f"Server started (PID {proc.pid}) but may not be ready yet.")
    print(f"Check logs: tail -f {SERVER_LOG_FILE}")


def cmd_server_stop(args):
    """Stop the background server."""
    if not SERVER_PID_FILE.exists():
        print("Server not running (no PID file)")
        return

    pid = int(SERVER_PID_FILE.read_text().strip())
    try:
        os.kill(pid, signal.SIGTERM)
        print(f"Server stopped (PID {pid})")
    except OSError as e:
        print(f"Could not stop server: {e}")

    SERVER_PID_FILE.unlink(missing_ok=True)


def cmd_server_status(args):
    """Check server status."""
    if SERVER_PID_FILE.exists():
        pid = int(SERVER_PID_FILE.read_text().strip())
        try:
            os.kill(pid, 0)
            print(f"Server running (PID {pid})")
            print(f"  URL: {DEFAULT_SERVER_URL}")
        except OSError:
            print("Server not running (stale PID file)")
    else:
        print("Server not running")


# =============================================================================
# Login
# =============================================================================

def cmd_login(args):
    """Login to the Corely server with 2FA."""
    server = args.server_url or _server_url

    print(f"Logging in to {server}")
    print()

    # Get username and password
    username = input("Username: ").strip()
    password = getpass.getpass("Password: ")

    # Step 1: Authenticate with username/password
    try:
        with httpx.Client(timeout=30.0) as client:
            response = client.post(
                f"{server}/api/auth/login",
                data={"username": username, "password": password},
            )

            if response.status_code == 401:
                print("Error: Invalid username or password")
                sys.exit(1)

            response.raise_for_status()
            data = response.json()

            if not data.get("requires_2fa"):
                # Old auth flow (shouldn't happen with new server)
                token = data.get("access_token")
            else:
                # 2FA required
                pending_token = data["pending_token"]
                print()
                code = getpass.getpass("Verification code: ").strip()

                # Step 2: Verify 2FA code
                response = client.post(
                    f"{server}/api/auth/verify",
                    json={"pending_token": pending_token, "code": code},
                )

                if response.status_code == 401:
                    print("Error: Invalid verification code")
                    sys.exit(1)

                response.raise_for_status()
                data = response.json()
                token = data["access_token"]

    except httpx.ConnectError:
        print(f"Error: Could not connect to {server}")
        sys.exit(1)
    except httpx.HTTPStatusError as e:
        print(f"Error: {e}")
        sys.exit(1)

    # Save token to config
    config = load_config()
    if "tokens" not in config:
        config["tokens"] = {}
    config["tokens"][server] = token
    save_config(config)

    print()
    print(f"Logged in successfully!")
    print(f"Token saved to {CONFIG_FILE}")


def cmd_logout(args):
    """Logout from the Corely server."""
    server = args.server_url or _server_url

    config = load_config()
    tokens = config.get("tokens", {})

    if server in tokens:
        del tokens[server]
        config["tokens"] = tokens
        save_config(config)
        print(f"Logged out from {server}")
    else:
        print(f"Not logged in to {server}")


# =============================================================================
# Machine Listing
# =============================================================================

def cmd_ls(args):
    """List all machines."""
    client = get_client()
    workers = client.list_workers()

    if not workers:
        print("No workers registered")
        return

    # Format output
    print(f"{'NAME':<20} {'ID':<12} {'STATUS':<10} {'OS':<15} {'HOST'}")
    print("-" * 80)

    for w in workers:
        status = "\033[32m●\033[0m online" if w.is_online else "\033[31m○\033[0m offline"
        os_info = f"{w.os or '?'}/{w.arch or '?'}"
        print(f"{w.name:<20} {w.id[:10]:<12} {status:<18} {os_info:<15} {w.hostname or '-'}")


# =============================================================================
# Interactive Shell (SSH-like)
# =============================================================================

def cmd_ssh(args):
    """Start an interactive shell session."""
    client = get_client()
    worker_id = resolve_worker(client, args.machine)

    worker = client.get_worker(worker_id)
    if not worker.is_online:
        print(f"Error: Worker '{worker.name}' is offline")
        sys.exit(1)

    print(f"Connected to {worker.name} ({worker.hostname})")
    print("Type 'exit' or Ctrl+D to disconnect\n")

    # Create a PTY session
    result = client.session_create(worker_id, name=f"ssh-{os.getpid()}")
    session_id = result.get("session_id")

    try:
        # Simple REPL
        while True:
            try:
                # Read output first
                output = client.session_read(worker_id, session_id)
                if output.get("output"):
                    print(output["output"], end="", flush=True)

                # Get input
                line = input()

                if line.strip().lower() == "exit":
                    break

                # Send input with newline
                client.session_input(worker_id, session_id, line + "\n")

                # Small delay and read output
                time.sleep(0.1)
                output = client.session_read(worker_id, session_id)
                if output.get("output"):
                    print(output["output"], end="", flush=True)

            except EOFError:
                break
            except KeyboardInterrupt:
                # Send Ctrl+C
                client.session_key(worker_id, session_id, "ctrl-c")

    finally:
        client.session_kill(worker_id, session_id)
        print("\nDisconnected")


# =============================================================================
# Tmux-like Session Management
# =============================================================================

def cmd_tmux(args):
    """Tmux-like session management."""
    client = get_client()
    worker_id = resolve_worker(client, args.machine)

    if args.tmux_cmd == "ls":
        # List sessions
        result = client.session_list(worker_id)
        sessions = result.get("sessions", [])

        if not sessions:
            print("No sessions")
            return

        print(f"{'NAME':<20} {'ID':<12} {'SIZE':<10} {'UPTIME'}")
        print("-" * 60)
        for s in sessions:
            uptime = f"{s.get('uptime_secs', 0)}s"
            print(f"{s['name']:<20} {s['id'][:10]:<12} {s.get('size', '?'):<10} {uptime}")

    elif args.tmux_cmd == "new":
        # Create new session
        name = args.session_name or f"session-{os.getpid()}"
        result = client.session_create(worker_id, name=name)
        print(f"Created session: {result.get('session_id')}")

        if not args.detach:
            # Attach to session
            _attach_session(client, worker_id, result.get("session_id"))

    elif args.tmux_cmd == "attach" or args.tmux_cmd == "a":
        # Attach to existing session
        if not args.session_name:
            # Get first session
            result = client.session_list(worker_id)
            sessions = result.get("sessions", [])
            if not sessions:
                print("No sessions to attach to")
                sys.exit(1)
            session_id = sessions[0]["id"]
        else:
            session_id = args.session_name

        _attach_session(client, worker_id, session_id)

    elif args.tmux_cmd == "kill":
        # Kill session
        if not args.session_name:
            print("Usage: corely tmux <machine> kill <session>")
            sys.exit(1)
        client.session_kill(worker_id, args.session_name)
        print(f"Killed session: {args.session_name}")

    else:
        # Default: attach or create
        result = client.session_list(worker_id)
        sessions = result.get("sessions", [])

        if sessions:
            _attach_session(client, worker_id, sessions[0]["id"])
        else:
            result = client.session_create(worker_id)
            _attach_session(client, worker_id, result.get("session_id"))


def _attach_session(client: CorelyClient, worker_id: str, session_id: str):
    """Attach to a session with full terminal support."""
    import select
    import termios
    import tty

    print(f"Attached to session {session_id[:8]}...")
    print("Press Ctrl+B D to detach\n")

    old_settings = termios.tcgetattr(sys.stdin)

    try:
        tty.setraw(sys.stdin.fileno())

        ctrl_b_pressed = False

        while True:
            # Check for input
            if select.select([sys.stdin], [], [], 0.1)[0]:
                char = sys.stdin.read(1)

                # Handle Ctrl+B D (detach)
                if ctrl_b_pressed:
                    if char.lower() == 'd':
                        break
                    ctrl_b_pressed = False
                    # Send the buffered Ctrl+B
                    client.session_input(worker_id, session_id, '\x02')

                if char == '\x02':  # Ctrl+B
                    ctrl_b_pressed = True
                    continue

                # Send character
                client.session_input(worker_id, session_id, char)

            # Read output
            try:
                output = client.session_read(worker_id, session_id)
                if output.get("output"):
                    sys.stdout.write(output["output"])
                    sys.stdout.flush()
            except:
                pass

    finally:
        termios.tcsetattr(sys.stdin, termios.TCSADRAIN, old_settings)
        print("\nDetached")


# =============================================================================
# Claude Code-like Session
# =============================================================================

def cmd_code(args):
    """Start a Claude Code-like session on a remote machine."""
    client = get_client()
    worker_id = resolve_worker(client, args.machine)

    worker = client.get_worker(worker_id)
    if not worker.is_online:
        print(f"Error: Worker '{worker.name}' is offline")
        sys.exit(1)

    path = args.path or "~"

    print(f"Corely Code - {worker.name}:{path}")
    print("=" * 60)
    print("Interactive AI coding session on remote machine")
    print("Commands: /help, /ls, /cat <file>, /edit <file>, /run <cmd>, /exit")
    print("=" * 60)
    print()

    # Expand path
    if path == "~":
        result = client.bash(worker_id, "echo $HOME")
        path = result.stdout.strip()

    cwd = path
    history = []

    while True:
        try:
            prompt = f"\033[36m{worker.name}\033[0m:\033[33m{cwd}\033[0m $ "
            line = input(prompt).strip()

            if not line:
                continue

            history.append(line)

            if line == "/exit" or line == "exit":
                break

            elif line == "/help":
                print("""
Commands:
  /ls [path]           List files
  /cat <file>          Show file contents
  /edit <file>         Edit a file (opens in local editor)
  /run <command>       Run a shell command
  /cd <path>           Change directory
  /pwd                 Print working directory
  /upload <local> <remote>  Upload a file
  /download <remote> <local> Download a file
  /exit                Exit session

Or just type a shell command to execute it.
""")

            elif line.startswith("/ls"):
                parts = line.split(maxsplit=1)
                target = parts[1] if len(parts) > 1 else cwd
                result = client.bash(worker_id, f"ls -la {target}", cwd=cwd)
                print(result.stdout)
                if result.stderr:
                    print(result.stderr, file=sys.stderr)

            elif line.startswith("/cat "):
                file_path = line[5:].strip()
                try:
                    content = client.read(worker_id, file_path if file_path.startswith("/") else f"{cwd}/{file_path}")
                    print(content.content)
                except Exception as e:
                    print(f"Error: {e}")

            elif line.startswith("/edit "):
                file_path = line[6:].strip()
                full_path = file_path if file_path.startswith("/") else f"{cwd}/{file_path}"

                # Download file
                try:
                    content = client.read(worker_id, full_path)
                    original = content.content
                except:
                    original = ""

                # Edit locally
                with tempfile.NamedTemporaryFile(mode="w", suffix=Path(file_path).suffix, delete=False) as f:
                    f.write(original)
                    temp_path = f.name

                editor = os.environ.get("EDITOR", "nano")
                subprocess.run([editor, temp_path])

                # Upload changes
                with open(temp_path) as f:
                    new_content = f.read()

                os.unlink(temp_path)

                if new_content != original:
                    client.write(worker_id, full_path, new_content)
                    print(f"Saved {file_path}")
                else:
                    print("No changes")

            elif line.startswith("/run "):
                cmd = line[5:].strip()
                result = client.bash(worker_id, cmd, cwd=cwd, timeout=60000)
                if result.stdout:
                    print(result.stdout)
                if result.stderr:
                    print(result.stderr, file=sys.stderr)
                if result.exit_code != 0:
                    print(f"Exit code: {result.exit_code}")

            elif line.startswith("/cd "):
                new_path = line[4:].strip()
                if new_path.startswith("/"):
                    test_path = new_path
                elif new_path == "~":
                    result = client.bash(worker_id, "echo $HOME")
                    test_path = result.stdout.strip()
                else:
                    test_path = f"{cwd}/{new_path}"

                # Verify path exists
                result = client.bash(worker_id, f"cd {test_path} && pwd")
                if result.exit_code == 0:
                    cwd = result.stdout.strip()
                else:
                    print(f"cd: {new_path}: No such directory")

            elif line == "/pwd":
                print(cwd)

            elif line.startswith("/upload "):
                parts = line.split()
                if len(parts) != 3:
                    print("Usage: /upload <local> <remote>")
                    continue
                local_path, remote_path = parts[1], parts[2]
                if not remote_path.startswith("/"):
                    remote_path = f"{cwd}/{remote_path}"

                with open(local_path) as f:
                    content = f.read()
                client.write(worker_id, remote_path, content)
                print(f"Uploaded {local_path} -> {remote_path}")

            elif line.startswith("/download "):
                parts = line.split()
                if len(parts) != 3:
                    print("Usage: /download <remote> <local>")
                    continue
                remote_path, local_path = parts[1], parts[2]
                if not remote_path.startswith("/"):
                    remote_path = f"{cwd}/{remote_path}"

                content = client.read(worker_id, remote_path)
                with open(local_path, "w") as f:
                    f.write(content.content)
                print(f"Downloaded {remote_path} -> {local_path}")

            elif line.startswith("/"):
                print(f"Unknown command: {line.split()[0]}")

            else:
                # Execute as shell command
                result = client.bash(worker_id, line, cwd=cwd, timeout=60000)
                if result.stdout:
                    print(result.stdout, end="")
                if result.stderr:
                    print(result.stderr, end="", file=sys.stderr)
                if result.exit_code != 0 and not result.stdout and not result.stderr:
                    print(f"Exit code: {result.exit_code}")

        except EOFError:
            break
        except KeyboardInterrupt:
            print()
            continue

    print("Goodbye!")


# =============================================================================
# SCP-like File Transfer
# =============================================================================

def cmd_cp(args):
    """SCP-like file transfer.

    Formats:
      corely cp local_file machine:remote_path      Upload
      corely cp machine:remote_path local_file      Download
      corely cp machine1:path machine2:path         Transfer between machines
    """
    client = get_client()

    # Parse source and destination
    src_machine, src_path = _parse_remote_path(args.source)
    dst_machine, dst_path = _parse_remote_path(args.destination)

    if src_machine and dst_machine:
        # Machine to machine transfer
        src_worker_id = resolve_worker(client, src_machine)
        dst_worker_id = resolve_worker(client, dst_machine)

        # Download then upload
        content = client.read(src_worker_id, src_path)
        client.write(dst_worker_id, dst_path, content.content)
        print(f"Transferred {src_machine}:{src_path} -> {dst_machine}:{dst_path}")

    elif src_machine:
        # Download from remote
        worker_id = resolve_worker(client, src_machine)

        # Handle recursive flag for directories
        if args.recursive:
            # Check if source is a directory
            result = client.bash(worker_id, f"test -d {src_path} && echo dir || echo file")
            if "dir" in result.stdout:
                _download_recursive(client, worker_id, src_path, dst_path)
                return

        content = client.read(worker_id, src_path)

        # Handle destination
        if os.path.isdir(dst_path):
            dst_path = os.path.join(dst_path, os.path.basename(src_path))

        with open(dst_path, "w") as f:
            f.write(content.content)

        print(f"Downloaded {src_machine}:{src_path} -> {dst_path}")

    elif dst_machine:
        # Upload to remote
        worker_id = resolve_worker(client, dst_machine)

        if args.recursive and os.path.isdir(args.source):
            _upload_recursive(client, worker_id, args.source, dst_path)
            return

        with open(args.source) as f:
            content = f.read()

        client.write(worker_id, dst_path, content)
        print(f"Uploaded {args.source} -> {dst_machine}:{dst_path}")

    else:
        print("Error: At least one path must be remote (machine:path)")
        sys.exit(1)


def _parse_remote_path(path: str) -> tuple[Optional[str], str]:
    """Parse a path that might be remote (machine:path)."""
    if ":" in path and not path.startswith("/"):
        parts = path.split(":", 1)
        return parts[0], parts[1]
    return None, path


def _download_recursive(client, worker_id, remote_path, local_path):
    """Recursively download a directory."""
    os.makedirs(local_path, exist_ok=True)

    # List files
    result = client.bash(worker_id, f"find {remote_path} -type f")
    files = result.stdout.strip().split("\n")

    for remote_file in files:
        if not remote_file:
            continue
        rel_path = os.path.relpath(remote_file, remote_path)
        local_file = os.path.join(local_path, rel_path)

        os.makedirs(os.path.dirname(local_file), exist_ok=True)

        content = client.read(worker_id, remote_file)
        with open(local_file, "w") as f:
            f.write(content.content)
        print(f"  {remote_file}")

    print(f"Downloaded {len(files)} files")


def _upload_recursive(client, worker_id, local_path, remote_path):
    """Recursively upload a directory."""
    # Create remote directory
    client.bash(worker_id, f"mkdir -p {remote_path}")

    count = 0
    for root, dirs, files in os.walk(local_path):
        rel_root = os.path.relpath(root, local_path)

        for d in dirs:
            remote_dir = f"{remote_path}/{rel_root}/{d}".replace("./", "")
            client.bash(worker_id, f"mkdir -p {remote_dir}")

        for f in files:
            local_file = os.path.join(root, f)
            remote_file = f"{remote_path}/{rel_root}/{f}".replace("./", "")

            with open(local_file) as fh:
                content = fh.read()

            client.write(worker_id, remote_file, content)
            print(f"  {local_file}")
            count += 1

    print(f"Uploaded {count} files")


# =============================================================================
# Execute Command
# =============================================================================

def cmd_exec(args):
    """Execute a command on a remote machine."""
    client = get_client()
    worker_id = resolve_worker(client, args.machine)

    command = " ".join(args.cmd_args)
    result = client.bash(worker_id, command, timeout=args.timeout * 1000)

    if result.stdout:
        print(result.stdout, end="")
    if result.stderr:
        print(result.stderr, end="", file=sys.stderr)

    sys.exit(result.exit_code)


# =============================================================================
# Uninstall
# =============================================================================

def cmd_rm(args):
    """Uninstall Corely from a remote machine."""
    client = get_client()
    worker_id = resolve_worker(client, args.machine)

    worker = client.get_worker(worker_id)

    if not args.force:
        confirm = input(f"Uninstall Corely from '{worker.name}'? This is irreversible. [y/N] ")
        if confirm.lower() != "y":
            print("Cancelled")
            return

    print(f"Uninstalling from {worker.name}...")
    try:
        result = client.uninstall(worker_id)
        print("Uninstall initiated. Worker will disconnect shortly.")
    except Exception as e:
        print(f"Error: {e}")
        sys.exit(1)


# =============================================================================
# OAuth Management
# =============================================================================

def cmd_oauth_create(args):
    """Create a new OAuth client for MCP access."""
    config = load_config()
    tokens = config.get("tokens", {})
    token = tokens.get(_server_url)

    if not token:
        print(f"Not logged in to {_server_url}")
        print("Run: corely login")
        sys.exit(1)

    name = args.name
    scopes = args.scopes.split(",") if args.scopes else ["read", "write"]

    try:
        with httpx.Client(timeout=30.0) as client:
            response = client.post(
                f"{_server_url}/api/oauth/clients",
                json={"name": name, "scopes": scopes},
                headers={"Authorization": f"Bearer {token}"},
            )

            if response.status_code == 403:
                print("Error: Admin access required to create OAuth clients")
                sys.exit(1)

            response.raise_for_status()
            data = response.json()

            print()
            print("OAuth client created successfully!")
            print()
            print(f"  Name:          {data['name']}")
            print(f"  Client ID:     {data['client_id']}")
            print(f"  Client Secret: {data['client_secret']}")
            print(f"  Scopes:        {', '.join(data['scopes'])}")
            print()
            print("⚠️  Save the client secret now - it won't be shown again!")
            print()
            print("To use with Claude Code, add to your settings.json:")
            print()
            print(f'''{{
  "mcpServers": {{
    "corely": {{
      "url": "{_server_url}/api/mcp/sse",
      "headers": {{
        "Authorization": "Bearer <access_token>"
      }}
    }}
  }}
}}''')
            print()
            print("First, get an access token:")
            print(f"  curl -X POST {_server_url}/api/oauth/token \\")
            print(f"    -H 'Content-Type: application/json' \\")
            print(f"    -d '{{\"client_id\": \"{data['client_id']}\", \"client_secret\": \"{data['client_secret']}\"}}'")

    except httpx.HTTPStatusError as e:
        print(f"Error: {e.response.text}")
        sys.exit(1)


def cmd_oauth_list(args):
    """List all OAuth clients."""
    config = load_config()
    tokens = config.get("tokens", {})
    token = tokens.get(_server_url)

    if not token:
        print(f"Not logged in to {_server_url}")
        print("Run: corely login")
        sys.exit(1)

    try:
        with httpx.Client(timeout=30.0) as client:
            response = client.get(
                f"{_server_url}/api/oauth/clients",
                headers={"Authorization": f"Bearer {token}"},
            )

            if response.status_code == 403:
                print("Error: Admin access required to list OAuth clients")
                sys.exit(1)

            response.raise_for_status()
            data = response.json()

            clients = data.get("clients", [])
            if not clients:
                print("No OAuth clients")
                return

            print(f"{'NAME':<20} {'CLIENT ID':<40} {'SCOPES':<15} {'LAST USED'}")
            print("-" * 95)

            for c in clients:
                last_used = c.get("last_used", "Never") or "Never"
                if last_used != "Never":
                    last_used = last_used[:19]  # Truncate ISO timestamp
                scopes = ", ".join(c["scopes"])
                print(f"{c['name']:<20} {c['client_id']:<40} {scopes:<15} {last_used}")

    except httpx.HTTPStatusError as e:
        print(f"Error: {e.response.text}")
        sys.exit(1)


def cmd_oauth_revoke(args):
    """Revoke an OAuth client."""
    config = load_config()
    tokens = config.get("tokens", {})
    token = tokens.get(_server_url)

    if not token:
        print(f"Not logged in to {_server_url}")
        print("Run: corely login")
        sys.exit(1)

    client_id = args.client_id

    if not args.force:
        confirm = input(f"Revoke OAuth client '{client_id}'? [y/N] ")
        if confirm.lower() != "y":
            print("Cancelled")
            return

    try:
        with httpx.Client(timeout=30.0) as client:
            response = client.delete(
                f"{_server_url}/api/oauth/clients/{client_id}",
                headers={"Authorization": f"Bearer {token}"},
            )

            if response.status_code == 403:
                print("Error: Admin access required to revoke OAuth clients")
                sys.exit(1)
            elif response.status_code == 404:
                print(f"Error: Client not found: {client_id}")
                sys.exit(1)

            response.raise_for_status()
            print(f"OAuth client revoked: {client_id}")

    except httpx.HTTPStatusError as e:
        print(f"Error: {e.response.text}")
        sys.exit(1)


def cmd_mcp_url(args):
    """Show the MCP URL for this server."""
    print(f"MCP SSE URL: {_server_url}/api/mcp/sse")
    print()
    print("To get an access token, first create an OAuth client:")
    print("  corely oauth create --name 'My MCP Client'")
    print()
    print("Then exchange credentials for a token:")
    print(f"  curl -X POST {_server_url}/api/oauth/token \\")
    print("    -H 'Content-Type: application/json' \\")
    print("    -d '{\"client_id\": \"<client_id>\", \"client_secret\": \"<client_secret>\"}'")


# =============================================================================
# Main
# =============================================================================

def main():
    parser = argparse.ArgumentParser(
        prog="corely",
        description="Corely - Remote machine management CLI",
    )
    parser.add_argument("--server", default=DEFAULT_SERVER_URL, help="Server URL")

    subparsers = parser.add_subparsers(dest="command", help="Commands")

    # Login/Logout
    login_parser = subparsers.add_parser("login", help="Login to server")
    login_parser.add_argument("server_url", nargs="?", help="Server URL (optional)")

    logout_parser = subparsers.add_parser("logout", help="Logout from server")
    logout_parser.add_argument("server_url", nargs="?", help="Server URL (optional)")

    # Server commands
    server_parser = subparsers.add_parser("server", help="Server management")
    server_sub = server_parser.add_subparsers(dest="server_cmd")
    server_sub.add_parser("start", help="Start server in background")
    server_sub.add_parser("stop", help="Stop background server")
    server_sub.add_parser("status", help="Check server status")

    # List machines
    ls_parser = subparsers.add_parser("ls", help="List machines")

    # SSH-like session
    ssh_parser = subparsers.add_parser("ssh", help="Interactive shell session")
    ssh_parser.add_argument("machine", help="Machine name or ID")

    # Tmux-like sessions
    tmux_parser = subparsers.add_parser("tmux", help="Tmux-like session management")
    tmux_parser.add_argument("machine", help="Machine name or ID")
    tmux_parser.add_argument("tmux_cmd", nargs="?", default=None,
                            help="Command: ls, new, attach, kill")
    tmux_parser.add_argument("session_name", nargs="?", help="Session name or ID")
    tmux_parser.add_argument("-d", "--detach", action="store_true", help="Start detached")

    # Code session
    code_parser = subparsers.add_parser("code", help="Claude Code-like session")
    code_parser.add_argument("machine", help="Machine name or ID")
    code_parser.add_argument("path", nargs="?", default="~", help="Starting directory")

    # SCP-like copy
    cp_parser = subparsers.add_parser("cp", help="Copy files (SCP-like)")
    cp_parser.add_argument("source", help="Source path (machine:path or local)")
    cp_parser.add_argument("destination", help="Destination path (machine:path or local)")
    cp_parser.add_argument("-r", "--recursive", action="store_true", help="Copy directories recursively")

    # Execute command
    exec_parser = subparsers.add_parser("exec", help="Execute a command")
    exec_parser.add_argument("machine", help="Machine name or ID")
    exec_parser.add_argument("cmd_args", nargs="+", help="Command to execute")
    exec_parser.add_argument("-t", "--timeout", type=int, default=30, help="Timeout in seconds")

    # Uninstall
    rm_parser = subparsers.add_parser("rm", help="Uninstall worker from machine")
    rm_parser.add_argument("machine", help="Machine name or ID")
    rm_parser.add_argument("-f", "--force", action="store_true", help="Skip confirmation")

    # OAuth commands
    oauth_parser = subparsers.add_parser("oauth", help="OAuth client management for MCP")
    oauth_sub = oauth_parser.add_subparsers(dest="oauth_cmd")

    oauth_create = oauth_sub.add_parser("create", help="Create a new OAuth client")
    oauth_create.add_argument("--name", "-n", required=True, help="Client name")
    oauth_create.add_argument("--scopes", "-s", default="read,write", help="Comma-separated scopes (default: read,write)")

    oauth_sub.add_parser("list", help="List all OAuth clients")
    oauth_sub.add_parser("ls", help="List all OAuth clients (alias)")

    oauth_revoke = oauth_sub.add_parser("revoke", help="Revoke an OAuth client")
    oauth_revoke.add_argument("client_id", help="Client ID to revoke")
    oauth_revoke.add_argument("-f", "--force", action="store_true", help="Skip confirmation")

    # MCP URL
    subparsers.add_parser("mcp", help="Show MCP URL and setup instructions")

    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        sys.exit(0)

    # Update server URL from args
    global _server_url
    _server_url = args.server

    try:
        if args.command == "login":
            cmd_login(args)
        elif args.command == "logout":
            cmd_logout(args)
        elif args.command == "server":
            if args.server_cmd == "start":
                cmd_server_start(args)
            elif args.server_cmd == "stop":
                cmd_server_stop(args)
            elif args.server_cmd == "status":
                cmd_server_status(args)
            else:
                print("Usage: corely server [start|stop|status]")
        elif args.command == "ls":
            cmd_ls(args)
        elif args.command == "ssh":
            cmd_ssh(args)
        elif args.command == "tmux":
            cmd_tmux(args)
        elif args.command == "code":
            cmd_code(args)
        elif args.command == "cp":
            cmd_cp(args)
        elif args.command == "exec":
            cmd_exec(args)
        elif args.command == "rm":
            cmd_rm(args)
        elif args.command == "oauth":
            if args.oauth_cmd == "create":
                cmd_oauth_create(args)
            elif args.oauth_cmd in ("list", "ls"):
                cmd_oauth_list(args)
            elif args.oauth_cmd == "revoke":
                cmd_oauth_revoke(args)
            else:
                print("Usage: corely oauth [create|list|revoke]")
        elif args.command == "mcp":
            cmd_mcp_url(args)
        else:
            parser.print_help()
    except KeyboardInterrupt:
        print()
        sys.exit(130)
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
