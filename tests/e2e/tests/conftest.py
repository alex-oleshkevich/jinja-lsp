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


def open_doc(client, path: Path, version: int = 1) -> str:
    """didOpen `path` from disk and return its URI."""
    uri = path.as_uri()
    client.text_document_did_open(
        lsp.DidOpenTextDocumentParams(
            text_document=lsp.TextDocumentItem(
                uri=uri, language_id="jinja", version=version, text=path.read_text()
            )
        )
    )
    return uri


def open_source(client, uri: str, source: str, version: int = 1) -> str:
    """didOpen an in-memory document and return its URI."""
    client.text_document_did_open(
        lsp.DidOpenTextDocumentParams(
            text_document=lsp.TextDocumentItem(
                uri=uri, language_id="jinja", version=version, text=source
            )
        )
    )
    return uri


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


async def _start(lsp_client: pytest_lsp.LanguageClient, root_uri: str | None):
    """Initialize a session, optionally rooted at a workspace."""
    # Acknowledge dynamic capability registration so the server's
    # client/registerCapability request (for file watchers) doesn't error.
    @lsp_client.feature("client/registerCapability")
    def _handle_register_capability(params):  # noqa: F811
        pass

    params = lsp.InitializeParams(
        capabilities=lsp.ClientCapabilities(),
        root_uri=root_uri,
    )
    result = await lsp_client.initialize_session(params)
    # pytest-lsp 1.0.0 doesn't expose these as attributes; add them manually.
    lsp_client.server_capabilities = result.capabilities
    lsp_client.server_info = result.server_info
    return lsp_client


@pytest_lsp.fixture(
    config=pytest_lsp.ClientServerConfig(
        server_command=[str(BINARY), "lsp"],
    )
)
async def client(lsp_client: pytest_lsp.LanguageClient):
    """Start the jinja-lsp server with no workspace root."""
    yield await _start(lsp_client, None)
    await _shutdown(lsp_client)


@pytest_lsp.fixture(
    config=pytest_lsp.ClientServerConfig(
        server_command=[str(BINARY), "lsp"],
    )
)
async def blog_client(lsp_client: pytest_lsp.LanguageClient):
    """Start the server rooted at the starlette-blog fixture workspace.

    Cross-file features (references, call hierarchy, workspace symbols) resolve
    through the Pass 2 workspace index, which only has anything in it when the
    server was given a root to scan. Tests that assert on cross-file results
    must use this fixture, not `client`.
    """
    yield await _start(lsp_client, (FIXTURES / "starlette-blog").as_uri())
    await _shutdown(lsp_client)
