"""E2E tests for E01 architecture requirements.

REQ-ARCH-04: debounced Pass 2 with generation guard
REQ-ARCH-05: didSave triggers relink; didClose keeps file indexed
REQ-ARCH-06: config change triggers reload+relink, not Pass 1 on TOML
REQ-ARCH-08: capabilities declared in initialize response
"""
import pytest
import pytest_lsp
from lsprotocol import types as lsp

from conftest import FIXTURES, client  # noqa: F401


@pytest.mark.asyncio
async def test_initialize_declares_expected_capabilities(client):
    """REQ-ARCH-08: initialize response declares all expected providers."""
    caps = client.server_capabilities
    assert caps.text_document_sync is not None, "textDocumentSync must be declared"
    assert caps.completion_provider is not None, "completionProvider must be declared"
    assert caps.hover_provider is not None, "hoverProvider must be declared"
    assert caps.definition_provider is not None, "definitionProvider must be declared"
    assert caps.references_provider is not None, "referencesProvider must be declared"
    assert caps.document_symbol_provider is not None, "documentSymbolProvider must be declared"
    assert caps.document_highlight_provider is not None, "documentHighlightProvider must be declared"
    assert caps.folding_range_provider is not None, "foldingRangeProvider must be declared"
    assert caps.inlay_hint_provider is not None, "inlayHintProvider must be declared"
    assert caps.code_lens_provider is not None, "codeLensProvider must be declared"
    assert caps.code_action_provider is not None, "codeActionProvider must be declared"
    assert caps.document_formatting_provider is not None, "documentFormattingProvider must be declared"
    assert caps.document_range_formatting_provider is not None, "documentRangeFormattingProvider must be declared"


@pytest.mark.asyncio
async def test_did_open_does_not_crash(client):
    """REQ-ARCH-05: didOpen is handled without error."""
    base = FIXTURES / "starlette-blog" / "templates" / "base.html"
    client.text_document_did_open(
        lsp.DidOpenTextDocumentParams(
            text_document=lsp.TextDocumentItem(
                uri=base.as_uri(),
                language_id="jinja",
                version=1,
                text=base.read_text(),
            )
        )
    )
    # No exception means did_open was handled without crashing


@pytest.mark.asyncio
async def test_did_change_ignores_document_rejected_by_did_open(client):
    """jinja-lsp-n38o / REQ-EDIT-11: did_open rejects non-jinja/jinja-html languageIds,
    but did_change unconditionally ran Pass 1 for any URI. A document the server
    explicitly declined at open must not get indexed and linted on its first edit.
    """
    unclosed = FIXTURES / "syntax-errors" / "templates" / "unclosed_tag.html"
    uri = unclosed.as_uri()
    client.text_document_did_open(
        lsp.DidOpenTextDocumentParams(
            text_document=lsp.TextDocumentItem(
                uri=uri,
                language_id="html",  # not "jinja"/"jinja-html" — did_open must reject this
                version=1,
                text=unclosed.read_text(),
            )
        )
    )
    # Edit the still-broken content — if did_change indexed it despite the languageId
    # rejection, this content is guaranteed to produce a JINJA-E001 diagnostic.
    client.text_document_did_change(
        lsp.DidChangeTextDocumentParams(
            text_document=lsp.VersionedTextDocumentIdentifier(uri=uri, version=2),
            content_changes=[
                lsp.TextDocumentContentChangeWholeDocument(text=unclosed.read_text())
            ],
        )
    )
    # Give the server a moment to (incorrectly) process the change, if it were going to.
    import asyncio
    await asyncio.sleep(0.3)
    assert uri not in client.diagnostics or list(client.diagnostics[uri]) == [], (
        f"document rejected at did_open must not be indexed/linted by did_change: "
        f"{client.diagnostics.get(uri)}"
    )


@pytest.mark.asyncio
async def test_did_close_does_not_crash(client):
    """REQ-ARCH-05: didClose is handled; file stays indexed."""
    base = FIXTURES / "starlette-blog" / "templates" / "base.html"
    uri = base.as_uri()
    client.text_document_did_open(
        lsp.DidOpenTextDocumentParams(
            text_document=lsp.TextDocumentItem(
                uri=uri,
                language_id="jinja",
                version=1,
                text=base.read_text(),
            )
        )
    )
    client.text_document_did_close(
        lsp.DidCloseTextDocumentParams(
            text_document=lsp.TextDocumentIdentifier(uri=uri)
        )
    )
    # No exception; file is still in index (verified by server state, not inspectable here)
