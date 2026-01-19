"""Main entry point for Corely server."""

import argparse
import asyncio
import os
import sys
from contextlib import asynccontextmanager

import uvicorn
from fastapi import FastAPI, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import PlainTextResponse

from .routes import router
from .storage import storage
from .mcp import run_mcp_server
from .installer import generate_install_script, DEFAULT_PASSWORD
from .auth import WORKER_TOKEN


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Application lifespan handler."""
    # Startup
    await storage.init()
    yield
    # Shutdown
    await storage.close()


def create_app() -> FastAPI:
    """Create the FastAPI application."""
    app = FastAPI(
        title="Corely Server",
        description="Remote machine management server",
        version="0.1.0",
        lifespan=lifespan,
    )

    # CORS for web UI
    app.add_middleware(
        CORSMiddleware,
        allow_origins=["*"],  # Configure properly in production
        allow_credentials=True,
        allow_methods=["*"],
        allow_headers=["*"],
    )

    # Root-level install script (for curl -fsSL https://server/install.sh | bash)
    @app.get("/install.sh", response_class=PlainTextResponse)
    async def root_install_script(request: Request):
        """
        Generate and serve the install script at root level.

        Usage: curl -fsSL https://your-server/install.sh | bash
        """
        scheme = request.headers.get("x-forwarded-proto", request.url.scheme)
        host = request.headers.get("x-forwarded-host", request.headers.get("host", "localhost"))
        server_url = f"{scheme}://{host}"

        password = os.environ.get("CORELY_INSTALL_PASSWORD", DEFAULT_PASSWORD)
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

    # Health check at root
    @app.get("/health")
    async def health():
        return {"status": "ok"}

    app.include_router(router, prefix="/api")

    return app


def main():
    """Main entry point."""
    parser = argparse.ArgumentParser(description="Corely Server")
    parser.add_argument(
        "--mode",
        choices=["http", "mcp"],
        default="http",
        help="Server mode: http for REST API, mcp for MCP protocol",
    )
    parser.add_argument("--host", default="0.0.0.0", help="Host to bind to")
    parser.add_argument("--port", type=int, default=8000, help="Port to listen on")
    parser.add_argument("--reload", action="store_true", help="Enable auto-reload")

    args = parser.parse_args()

    if args.mode == "mcp":
        # Run MCP server over stdio
        asyncio.run(run_mcp_server())
    else:
        # Run HTTP server
        app = create_app()
        uvicorn.run(
            app,
            host=args.host,
            port=args.port,
            reload=args.reload,
        )


if __name__ == "__main__":
    main()
