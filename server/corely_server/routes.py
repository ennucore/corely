"""HTTP and WebSocket routes for Corely server."""

import asyncio
import hashlib
import json
import os
import secrets
from datetime import datetime, timedelta
from pathlib import Path
from typing import Any, Optional

from fastapi import APIRouter, Depends, HTTPException, Query, Request, WebSocket, WebSocketDisconnect, status
from fastapi.responses import PlainTextResponse, FileResponse, StreamingResponse
from fastapi.security import OAuth2PasswordRequestForm
from pydantic import BaseModel

from .auth import (
    ACCESS_TOKEN_EXPIRE_MINUTES,
    Token,
    User,
    WORKER_TOKEN,
    authenticate_user,
    consume_pending_token,
    create_access_token,
    create_pending_token,
    get_current_user,
    require_scope,
    verify_2fa_code,
    verify_worker_token,
)
from .installer import generate_install_script, DEFAULT_PASSWORD
from .storage import storage
from .worker_manager import WorkerInfo, worker_manager

router = APIRouter()

# Directory for worker binaries (configurable via environment)
BINARIES_DIR = Path(os.environ.get("CORELY_BINARIES_DIR", "./binaries"))


# ============================================================================
# Authentication Routes
# ============================================================================


class LoginRequest(BaseModel):
    username: str
    password: str


class PendingAuthResponse(BaseModel):
    pending_token: str
    requires_2fa: bool = True


class VerifyRequest(BaseModel):
    pending_token: str
    code: str


@router.post("/auth/login")
async def login(form_data: OAuth2PasswordRequestForm = Depends()):
    """
    Login step 1: Verify username/password and get a pending token.

    Returns a pending_token that must be verified with 2FA code.
    """
    user = await authenticate_user(form_data.username, form_data.password)
    if not user:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Incorrect username or password",
            headers={"WWW-Authenticate": "Bearer"},
        )

    # Create pending token for 2FA
    pending_token = create_pending_token(user.username)
    return {"pending_token": pending_token, "requires_2fa": True}


@router.post("/auth/verify", response_model=Token)
async def verify_2fa(request: VerifyRequest):
    """
    Login step 2: Verify 2FA code and get access token.
    """
    # Verify pending token
    username = consume_pending_token(request.pending_token)
    if not username:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Invalid or expired pending token",
        )

    # Verify 2FA code
    if not verify_2fa_code(request.code):
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Invalid verification code",
        )

    # Get user and create access token
    from .auth import get_user
    user = get_user(username)
    if not user:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="User not found",
        )

    access_token_expires = timedelta(minutes=ACCESS_TOKEN_EXPIRE_MINUTES)
    access_token = create_access_token(
        data={"sub": user.username, "scopes": user.scopes},
        expires_delta=access_token_expires,
    )
    return {"access_token": access_token, "token_type": "bearer"}


@router.get("/auth/me", response_model=User)
async def get_me(current_user: User = Depends(get_current_user)):
    """Get current user info."""
    return current_user


class PasswordChangeRequest(BaseModel):
    current_password: str
    new_password: str


class UsernameChangeRequest(BaseModel):
    new_username: str


@router.post("/auth/change-password")
async def change_password_route(
    request: PasswordChangeRequest,
    current_user: User = Depends(get_current_user),
):
    """Change the current user's password."""
    from .auth import authenticate_user, change_password

    # Verify current password
    user = await authenticate_user(current_user.username, request.current_password)
    if not user:
        raise HTTPException(status_code=400, detail="Current password is incorrect")

    # Change password
    success = await change_password(current_user.username, request.new_password)
    if not success:
        raise HTTPException(status_code=500, detail="Failed to change password")

    return {"message": "Password changed successfully"}


@router.post("/auth/change-username")
async def change_username_route(
    request: UsernameChangeRequest,
    current_user: User = Depends(get_current_user),
):
    """Change the current user's username."""
    from .auth import change_username

    if request.new_username == current_user.username:
        return {"message": "Username unchanged"}

    success = await change_username(current_user.username, request.new_username)
    if not success:
        raise HTTPException(status_code=400, detail="Username already taken or invalid")

    # Generate new token with new username
    access_token = create_access_token(
        data={"sub": request.new_username, "scopes": current_user.scopes}
    )

    return {
        "message": "Username changed successfully",
        "new_username": request.new_username,
        "access_token": access_token,
        "token_type": "bearer",
    }


# ============================================================================
# OAuth Client Management Routes
# ============================================================================


def hash_client_secret(secret: str) -> str:
    """Hash a client secret using SHA-256."""
    return hashlib.sha256(secret.encode()).hexdigest()


def generate_client_credentials() -> tuple[str, str]:
    """Generate a new client ID and secret."""
    client_id = f"corely_{secrets.token_hex(16)}"
    client_secret = secrets.token_urlsafe(32)
    return client_id, client_secret


class OAuthClientCreate(BaseModel):
    name: str
    scopes: list[str] = ["read", "write"]


class OAuthClientResponse(BaseModel):
    client_id: str
    client_secret: Optional[str] = None  # Only returned on creation
    name: str
    scopes: list[str]
    created_at: str
    created_by: str
    last_used: Optional[str] = None


class OAuthTokenRequest(BaseModel):
    client_id: str
    client_secret: str


class OAuthTokenResponse(BaseModel):
    access_token: str
    token_type: str = "bearer"
    expires_in: int = 7776000  # 90 days


@router.post("/oauth/clients", response_model=OAuthClientResponse)
async def create_oauth_client(
    request: OAuthClientCreate,
    current_user: User = Depends(require_scope("admin")),
):
    """
    Create a new OAuth client for MCP access.

    Returns the client_id and client_secret. The secret is only shown once.
    """
    client_id, client_secret = generate_client_credentials()
    secret_hash = hash_client_secret(client_secret)

    await storage.create_oauth_client(
        client_id=client_id,
        client_secret_hash=secret_hash,
        name=request.name,
        scopes=request.scopes,
        created_by=current_user.username,
    )

    return OAuthClientResponse(
        client_id=client_id,
        client_secret=client_secret,  # Only returned on creation
        name=request.name,
        scopes=request.scopes,
        created_at=datetime.utcnow().isoformat(),
        created_by=current_user.username,
    )


@router.get("/oauth/clients")
async def list_oauth_clients(current_user: User = Depends(require_scope("admin"))):
    """List all OAuth clients."""
    clients = await storage.get_all_oauth_clients()
    return {"clients": clients}


@router.delete("/oauth/clients/{client_id}")
async def revoke_oauth_client(
    client_id: str,
    current_user: User = Depends(require_scope("admin")),
):
    """Revoke an OAuth client."""
    client = await storage.get_oauth_client(client_id)
    if not client:
        raise HTTPException(status_code=404, detail="Client not found")

    await storage.revoke_oauth_client(client_id)
    return {"status": "revoked"}


@router.post("/oauth/token", response_model=OAuthTokenResponse)
async def oauth_token(request: OAuthTokenRequest):
    """
    Exchange client credentials for an access token.

    This is the OAuth 2.0 client credentials flow.
    """
    client = await storage.get_oauth_client(request.client_id)
    if not client:
        raise HTTPException(
            status_code=401,
            detail="Invalid client credentials",
        )

    # Verify secret
    if hash_client_secret(request.client_secret) != client["client_secret_hash"]:
        raise HTTPException(
            status_code=401,
            detail="Invalid client credentials",
        )

    # Update last used
    await storage.update_oauth_client_last_used(request.client_id)

    # Generate access token (90 days)
    access_token = create_access_token(
        data={"sub": f"oauth:{request.client_id}", "scopes": client["scopes"]},
        expires_delta=timedelta(days=90),
    )

    return OAuthTokenResponse(
        access_token=access_token,
        token_type="bearer",
        expires_in=7776000,  # 90 days in seconds
    )


# ============================================================================
# MCP over SSE Routes
# ============================================================================


async def verify_oauth_token(authorization: str = None) -> dict:
    """Verify OAuth bearer token and return client info."""
    from .auth import SECRET_KEY, ALGORITHM
    from jose import jwt, JWTError

    if not authorization or not authorization.startswith("Bearer "):
        raise HTTPException(status_code=401, detail="Missing or invalid authorization header")

    token = authorization[7:]  # Remove "Bearer "

    try:
        payload = jwt.decode(token, SECRET_KEY, algorithms=[ALGORITHM])
        return payload
    except JWTError:
        raise HTTPException(status_code=401, detail="Invalid token")


@router.get("/mcp/sse")
async def mcp_sse_endpoint(request: Request, authorization: str = Query(None, alias="token")):
    """
    MCP server endpoint using Server-Sent Events.

    This endpoint provides the MCP protocol over HTTP using SSE for server-to-client
    messages and POST requests for client-to-server messages.

    Connect with: GET /api/mcp/sse?token=<bearer_token>
    Send messages: POST /api/mcp/message?token=<bearer_token>
    """
    # Also accept Authorization header
    auth_header = request.headers.get("Authorization")
    if auth_header:
        authorization = auth_header.replace("Bearer ", "")

    if not authorization:
        raise HTTPException(status_code=401, detail="Missing authorization token")

    # Verify token
    from .auth import SECRET_KEY, ALGORITHM
    from jose import jwt, JWTError

    try:
        payload = jwt.decode(authorization, SECRET_KEY, algorithms=[ALGORITHM])
    except JWTError:
        raise HTTPException(status_code=401, detail="Invalid token")

    # Create a unique session ID for this SSE connection
    session_id = secrets.token_hex(16)

    # Store pending responses for this session
    if not hasattr(request.app.state, "mcp_sessions"):
        request.app.state.mcp_sessions = {}

    request.app.state.mcp_sessions[session_id] = {
        "queue": asyncio.Queue(),
        "payload": payload,
    }

    async def event_generator():
        """Generate SSE events."""
        try:
            # Send session ID as first event
            yield f"event: session\ndata: {json.dumps({'session_id': session_id})}\n\n"

            # Send server info
            server_info = {
                "jsonrpc": "2.0",
                "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05",
                    "serverInfo": {
                        "name": "corely",
                        "version": "0.1.0",
                    },
                    "capabilities": {
                        "tools": {},
                    },
                },
            }
            yield f"event: message\ndata: {json.dumps(server_info)}\n\n"

            queue = request.app.state.mcp_sessions[session_id]["queue"]

            while True:
                # Check if client disconnected
                if await request.is_disconnected():
                    break

                try:
                    # Wait for messages with timeout
                    message = await asyncio.wait_for(queue.get(), timeout=30.0)
                    yield f"event: message\ndata: {json.dumps(message)}\n\n"
                except asyncio.TimeoutError:
                    # Send keepalive
                    yield f"event: ping\ndata: {{}}\n\n"

        finally:
            # Cleanup session
            if session_id in request.app.state.mcp_sessions:
                del request.app.state.mcp_sessions[session_id]

    return StreamingResponse(
        event_generator(),
        media_type="text/event-stream",
        headers={
            "Cache-Control": "no-cache",
            "Connection": "keep-alive",
            "X-Accel-Buffering": "no",
        },
    )


class MCPMessage(BaseModel):
    jsonrpc: str = "2.0"
    id: Optional[Any] = None
    method: Optional[str] = None
    params: Optional[dict] = None
    result: Optional[Any] = None
    error: Optional[dict] = None


@router.post("/mcp/message")
async def mcp_message_endpoint(
    request: Request,
    message: MCPMessage,
    session_id: str = Query(...),
    authorization: str = Query(None, alias="token"),
):
    """
    Handle MCP messages from the client.

    Send JSON-RPC messages to this endpoint, referencing the session_id from the SSE connection.
    """
    # Also accept Authorization header
    auth_header = request.headers.get("Authorization")
    if auth_header:
        authorization = auth_header.replace("Bearer ", "")

    if not authorization:
        raise HTTPException(status_code=401, detail="Missing authorization token")

    # Verify token
    from .auth import SECRET_KEY, ALGORITHM
    from jose import jwt, JWTError

    try:
        jwt.decode(authorization, SECRET_KEY, algorithms=[ALGORITHM])
    except JWTError:
        raise HTTPException(status_code=401, detail="Invalid token")

    # Get session
    if not hasattr(request.app.state, "mcp_sessions"):
        raise HTTPException(status_code=404, detail="Session not found")

    session = request.app.state.mcp_sessions.get(session_id)
    if not session:
        raise HTTPException(status_code=404, detail="Session not found")

    # Handle the message
    from .mcp import get_dynamic_tools, call_tool

    response = None

    if message.method == "tools/list":
        tools = await get_dynamic_tools()
        response = {
            "jsonrpc": "2.0",
            "id": message.id,
            "result": {
                "tools": [
                    {
                        "name": t.name,
                        "description": t.description,
                        "inputSchema": t.inputSchema,
                    }
                    for t in tools
                ],
            },
        }

    elif message.method == "tools/call":
        tool_name = message.params.get("name")
        tool_args = message.params.get("arguments", {})

        try:
            result = await call_tool(tool_name, tool_args)
            response = {
                "jsonrpc": "2.0",
                "id": message.id,
                "result": {
                    "content": [{"type": "text", "text": r.text} for r in result],
                },
            }
        except Exception as e:
            response = {
                "jsonrpc": "2.0",
                "id": message.id,
                "error": {
                    "code": -32000,
                    "message": str(e),
                },
            }

    elif message.method == "initialize":
        response = {
            "jsonrpc": "2.0",
            "id": message.id,
            "result": {
                "protocolVersion": "2024-11-05",
                "serverInfo": {
                    "name": "corely",
                    "version": "0.1.0",
                },
                "capabilities": {
                    "tools": {},
                },
            },
        }

    elif message.method == "notifications/initialized":
        # Client notification, no response needed
        return {"status": "ok"}

    else:
        response = {
            "jsonrpc": "2.0",
            "id": message.id,
            "error": {
                "code": -32601,
                "message": f"Method not found: {message.method}",
            },
        }

    # Queue response for SSE
    if response and message.id is not None:
        await session["queue"].put(response)

    return {"status": "ok"}


# ============================================================================
# Install Script Routes (No Auth Required)
# ============================================================================


@router.get("/install.sh", response_class=PlainTextResponse)
async def get_install_script(request: Request):
    """
    Generate and serve the install script.

    The script contains the worker token encrypted with AES-256-CBC.
    Default password is 'cor4ly-p4ssword' - change CORELY_INSTALL_PASSWORD env var.

    Usage: curl -fsSL https://your-server/install.sh | bash
    """
    # Get server URL from request
    scheme = request.headers.get("x-forwarded-proto", request.url.scheme)
    host = request.headers.get("x-forwarded-host", request.headers.get("host", "localhost"))
    server_url = f"{scheme}://{host}"

    # Get encryption password from environment (default: cor4ly-p4ssword)
    password = os.environ.get("CORELY_INSTALL_PASSWORD", DEFAULT_PASSWORD)

    # Get worker token from environment or use default
    worker_token = os.environ.get("CORELY_WORKER_TOKEN", WORKER_TOKEN)

    script = generate_install_script(
        server_url=server_url,
        worker_token=worker_token,
        encryption_password=password,
    )

    return PlainTextResponse(
        content=script,
        media_type="text/x-shellscript",
        headers={"Content-Disposition": "inline; filename=install.sh"},
    )


@router.get("/downloads/{binary_name}")
async def download_binary(binary_name: str):
    """
    Serve worker binaries for different platforms.

    Expected binary names:
    - corely-worker-linux-x86_64
    - corely-worker-linux-aarch64
    - corely-worker-macos-x86_64
    - corely-worker-macos-aarch64
    - corely-worker-windows-x86_64.exe
    """
    # Validate binary name to prevent path traversal
    if "/" in binary_name or "\\" in binary_name or ".." in binary_name:
        raise HTTPException(status_code=400, detail="Invalid binary name")

    binary_path = BINARIES_DIR / binary_name

    if not binary_path.exists():
        raise HTTPException(
            status_code=404,
            detail=f"Binary not found: {binary_name}. Available binaries are served from {BINARIES_DIR}",
        )

    return FileResponse(
        path=binary_path,
        filename=binary_name,
        media_type="application/octet-stream",
    )


@router.get("/worker/version", response_class=PlainTextResponse)
async def get_worker_version():
    """
    Get the current stable worker version.
    Used by workers to check if an update is available.
    """
    version_file = BINARIES_DIR / "VERSION"
    if version_file.exists():
        return version_file.read_text().strip()

    # Default version if no VERSION file exists
    return "0.1.0"


# ============================================================================
# Worker Management Routes
# ============================================================================


@router.get("/workers")
async def list_workers(current_user: User = Depends(get_current_user)):
    """List all workers (connected and historical)."""
    # Get from database
    db_workers = await storage.get_all_workers()

    # Get currently connected
    connected_ids = {w.id for w in await worker_manager.get_all_workers()}

    # Merge data
    workers = []
    for w in db_workers:
        w["is_online"] = w["id"] in connected_ids
        if w["capabilities"]:
            w["capabilities"] = json.loads(w["capabilities"])
        workers.append(w)

    return {"workers": workers}


@router.get("/workers/{worker_id}")
async def get_worker(worker_id: str, current_user: User = Depends(get_current_user)):
    """Get a specific worker."""
    worker = await storage.get_worker(worker_id)
    if not worker:
        raise HTTPException(status_code=404, detail="Worker not found")

    connected_workers = await worker_manager.get_all_workers()
    worker["is_online"] = any(w.id == worker_id for w in connected_workers)

    if worker["capabilities"]:
        worker["capabilities"] = json.loads(worker["capabilities"])

    return worker


@router.patch("/workers/{worker_id}")
async def update_worker(
    worker_id: str,
    name: Optional[str] = None,
    current_user: User = Depends(require_scope("write")),
):
    """Update a worker's display name."""
    worker = await storage.get_worker(worker_id)
    if not worker:
        raise HTTPException(status_code=404, detail="Worker not found")

    if name:
        await storage.update_worker_name(worker_id, name)

    return {"status": "ok"}


@router.delete("/workers/{worker_id}")
async def delete_worker(
    worker_id: str, current_user: User = Depends(require_scope("admin"))
):
    """Delete a worker from the database."""
    await storage.delete_worker(worker_id)
    return {"status": "ok"}


# ============================================================================
# Worker Command Routes
# ============================================================================


class CommandRequest(BaseModel):
    method: str
    params: Optional[dict] = None
    timeout: float = 300.0


@router.post("/workers/{worker_id}/call")
async def call_worker(
    worker_id: str,
    request: CommandRequest,
    current_user: User = Depends(require_scope("write")),
):
    """Call a method on a worker."""
    connection = await worker_manager.get_worker(worker_id)
    if not connection:
        raise HTTPException(status_code=404, detail="Worker not connected")

    try:
        result = await worker_manager.call_worker(
            worker_id, request.method, request.params, request.timeout
        )
        return {"result": result}
    except asyncio.TimeoutError:
        raise HTTPException(status_code=504, detail="Worker request timed out")
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


# Convenience endpoints
@router.post("/workers/{worker_id}/shell")
async def worker_shell(
    worker_id: str,
    command: str,
    timeout: int = 30000,
    cwd: Optional[str] = None,
    current_user: User = Depends(require_scope("write")),
):
    """Execute a shell command on a worker."""
    connection = await worker_manager.get_worker(worker_id)
    if not connection:
        raise HTTPException(status_code=404, detail="Worker not connected")

    try:
        result = await worker_manager.call_worker(
            worker_id,
            "shell.exec",
            {"command": command, "timeout": timeout, "cwd": cwd},
        )
        return result
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@router.get("/workers/{worker_id}/screen")
async def worker_screen(
    worker_id: str,
    display_id: Optional[int] = None,
    current_user: User = Depends(require_scope("read")),
):
    """Capture screen from a worker."""
    connection = await worker_manager.get_worker(worker_id)
    if not connection:
        raise HTTPException(status_code=404, detail="Worker not connected")

    try:
        result = await worker_manager.call_worker(
            worker_id, "screen.capture", {"display_id": display_id}
        )
        return result
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@router.get("/workers/{worker_id}/system")
async def worker_system_info(
    worker_id: str, current_user: User = Depends(require_scope("read"))
):
    """Get system info from a worker."""
    connection = await worker_manager.get_worker(worker_id)
    if not connection:
        raise HTTPException(status_code=404, detail="Worker not connected")

    try:
        result = await worker_manager.call_worker(worker_id, "system.info", {})
        return result
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


@router.post("/workers/{worker_id}/uninstall")
async def worker_uninstall(
    worker_id: str, current_user: User = Depends(require_scope("admin"))
):
    """
    Completely uninstall Corely from a worker.

    WARNING: This action is irreversible. The worker will:
    - Stop and remove all autostart configurations
    - Remove configuration files and binaries
    - Terminate and disconnect

    Requires admin scope.
    """
    connection = await worker_manager.get_worker(worker_id)
    if not connection:
        raise HTTPException(status_code=404, detail="Worker not connected")

    try:
        result = await worker_manager.call_worker(worker_id, "system.uninstall", {})
        # Also remove from storage
        await storage.delete_worker(worker_id)
        return result
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


# ============================================================================
# WebSocket Routes
# ============================================================================


@router.websocket("/ws/worker")
async def worker_websocket(
    websocket: WebSocket, token: str = Query(...)
):
    """WebSocket endpoint for worker connections."""
    # Verify worker token
    if not verify_worker_token(token):
        await websocket.close(code=4001, reason="Invalid token")
        return

    await websocket.accept()
    connection = await worker_manager.register_worker(websocket)

    try:
        while True:
            data = await websocket.receive_text()
            message = json.loads(data)

            # Handle different message types
            method = message.get("method", "")

            if method == "worker.hello":
                await worker_manager.handle_hello(connection, message.get("params", {}))
            elif method == "worker.ping":
                # Just a heartbeat, update last seen
                if connection.info:
                    connection.info.last_seen = datetime.utcnow()
            elif "id" in message:
                # This is a response to a request
                await worker_manager.handle_response(connection, message)

    except WebSocketDisconnect:
        pass
    except Exception as e:
        print(f"Worker WebSocket error: {e}")
    finally:
        await worker_manager.unregister_worker(connection.worker_id)


@router.websocket("/ws/terminal/{worker_id}")
async def terminal_websocket(
    websocket: WebSocket,
    worker_id: str,
    token: str = Query(...),
):
    """WebSocket endpoint for terminal sessions (tmux)."""
    # Verify user token (extract from JWT)
    from .auth import SECRET_KEY, ALGORITHM
    from jose import jwt, JWTError

    try:
        payload = jwt.decode(token, SECRET_KEY, algorithms=[ALGORITHM])
        username = payload.get("sub")
        if not username:
            await websocket.close(code=4001, reason="Invalid token")
            return
    except JWTError:
        await websocket.close(code=4001, reason="Invalid token")
        return

    # Check worker is connected
    connection = await worker_manager.get_worker(worker_id)
    if not connection:
        await websocket.close(code=4004, reason="Worker not connected")
        return

    await websocket.accept()

    # Start a tmux session on the worker
    session_name = f"corely_{username}_{worker_id[:8]}"

    try:
        # Create or attach to tmux session
        await worker_manager.call_worker(
            worker_id,
            "shell.exec",
            {"command": f"tmux new-session -d -s {session_name} 2>/dev/null || true"},
        )

        while True:
            data = await websocket.receive_text()
            message = json.loads(data)

            if message.get("type") == "input":
                # Send input to tmux
                input_data = message.get("data", "")
                await worker_manager.call_worker(
                    worker_id,
                    "shell.exec",
                    {"command": f"tmux send-keys -t {session_name} '{input_data}'"},
                )
            elif message.get("type") == "resize":
                # Resize tmux window
                cols = message.get("cols", 80)
                rows = message.get("rows", 24)
                await worker_manager.call_worker(
                    worker_id,
                    "shell.exec",
                    {"command": f"tmux resize-window -t {session_name} -x {cols} -y {rows}"},
                )
            elif message.get("type") == "refresh":
                # Capture tmux output
                result = await worker_manager.call_worker(
                    worker_id,
                    "shell.exec",
                    {"command": f"tmux capture-pane -t {session_name} -p"},
                )
                await websocket.send_json(
                    {"type": "output", "data": result.get("stdout", "")}
                )

    except WebSocketDisconnect:
        pass
    except Exception as e:
        print(f"Terminal WebSocket error: {e}")


@router.websocket("/ws/screen/{worker_id}")
async def screen_stream_websocket(
    websocket: WebSocket,
    worker_id: str,
    token: str = Query(...),
):
    """WebSocket endpoint for live screen streaming."""
    from .auth import SECRET_KEY, ALGORITHM
    from jose import jwt, JWTError

    try:
        payload = jwt.decode(token, SECRET_KEY, algorithms=[ALGORITHM])
        if not payload.get("sub"):
            await websocket.close(code=4001, reason="Invalid token")
            return
    except JWTError:
        await websocket.close(code=4001, reason="Invalid token")
        return

    connection = await worker_manager.get_worker(worker_id)
    if not connection:
        await websocket.close(code=4004, reason="Worker not connected")
        return

    await websocket.accept()

    try:
        while True:
            # Wait for client requests
            data = await websocket.receive_text()
            message = json.loads(data)

            if message.get("type") == "capture":
                # Capture and send screen
                display_id = message.get("display_id")
                result = await worker_manager.call_worker(
                    worker_id, "screen.capture", {"display_id": display_id}
                )
                await websocket.send_json({"type": "frame", "data": result})

    except WebSocketDisconnect:
        pass
    except Exception as e:
        print(f"Screen stream WebSocket error: {e}")


# Need to import these for routes
import asyncio
from datetime import datetime
