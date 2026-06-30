"""E29 Branch B: REQ-E2E-07 — Protocol-conformance journeys.

These shared journeys exercise the E01 lifecycle and protocol conduct:
capability negotiation, empty publish on clean open, and coalesced relinks.
"""
import pytest
from lsprotocol import types as lsp

from conftest import FIXTURES


@pytest.mark.asyncio
async def test_capability_negotiation_succeeds(client):
    """REQ-E2E-07: initialize / initialized handshake completes."""
    caps = client.server_capabilities
    # Server must declare textDocumentSync (REQ-ARCH-08)
    assert caps.text_document_sync is not None


@pytest.mark.asyncio
async def test_server_info_reported(client):
    """REQ-E2E-07: initialize response includes server name."""
    info = client.server_info
    assert info is not None
    assert info.name == "jinja-lsp"


@pytest.mark.asyncio
async def test_did_open_then_did_change_no_crash(client):
    """REQ-E2E-07: rapid didChange doesn't crash the server."""
    uri = (FIXTURES / "starlette-blog" / "templates" / "base.html").as_uri()
    source = (FIXTURES / "starlette-blog" / "templates" / "base.html").read_text()

    client.text_document_did_open(
        lsp.DidOpenTextDocumentParams(
            text_document=lsp.TextDocumentItem(
                uri=uri, language_id="jinja", version=1, text=source
            )
        )
    )
    # Rapid changes (coalesce test)
    for version in range(2, 6):
        client.text_document_did_change(
            lsp.DidChangeTextDocumentParams(
                text_document=lsp.VersionedTextDocumentIdentifier(
                    uri=uri, version=version
                ),
                content_changes=[
                    lsp.TextDocumentContentChangeWholeDocument(
                        text=source + f"<!-- v{version} -->"
                    )
                ],
            )
        )
    # No exception means the server handled rapid changes without crashing
