"""pytest-lsp fixtures for jinja-lsp e2e tests.

REQ-ARCH-04, REQ-ARCH-05, REQ-ARCH-06, REQ-ARCH-08 are verified here.
"""
import asyncio
import os
from pathlib import Path

import pytest_lsp
from lsprotocol import types as lsp

# CI builds a --release binary and points JINJA_LSP_BINARY at it (see
# .github/workflows/ci.yml); fall back to the debug binary for local runs.
BINARY = Path(os.environ["JINJA_LSP_BINARY"]) if "JINJA_LSP_BINARY" in os.environ \
    else Path(__file__).parent.parent.parent.parent / "target" / "debug" / "jinja-lsp"
FIXTURES = Path(__file__).parent.parent.parent / "fixtures"


async def _shutdown(lsp_client: pytest_lsp.LanguageClient) -> None:
    """Shut down the server, forcing stdin EOF so tower-lsp actually exits.

    tower-lsp's Server::serve() only terminates when stdin reaches EOF.
    The standard shutdown_session() sends 'exit' but never closes subprocess
    stdin, causing await self._server.wait() to hang indefinitely.
    """
    if lsp_client.error is not None or lsp_client.capabilities is None:
        return

    await lsp_client.shutdown_async(None)
    lsp_client.exit(None)

    if lsp_client._server:
        if lsp_client._server.stdin:
            lsp_client._server.stdin.close()
        try:
            await asyncio.wait_for(lsp_client._server.wait(), timeout=5.0)
        except asyncio.TimeoutError:
            lsp_client._server.kill()
            await lsp_client._server.wait()


@pytest_lsp.fixture(
    config=pytest_lsp.ClientServerConfig(
        server_command=[str(BINARY), "lsp"],
    )
)
async def client(lsp_client: pytest_lsp.LanguageClient):
    """Start the jinja-lsp server and initialize it."""
    # Acknowledge dynamic capability registration so the server's
    # client/registerCapability request (for file watchers) doesn't error.
    @lsp_client.feature("client/registerCapability")
    def _handle_register_capability(params):  # noqa: F811
        pass

    params = lsp.InitializeParams(
        capabilities=lsp.ClientCapabilities(),
    )
    result = await lsp_client.initialize_session(params)
    # pytest-lsp 1.0.0 doesn't expose these as attributes; add them manually.
    lsp_client.server_capabilities = result.capabilities
    lsp_client.server_info = result.server_info

    yield lsp_client
    await _shutdown(lsp_client)
