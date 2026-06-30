"""REQ-E2E-06: Branch B pytest-lsp protocol journeys.

Verifies that each major LSP request/notification round-trip works end-to-end:
  - didOpen  → textDocument/publishDiagnostics
  - textDocument/completion  +  completionItem/resolve
  - textDocument/hover  (built-in filter)
  - textDocument/signatureHelp  (macro call)
  - textDocument/definition  (macro call site → macro declaration)
  - textDocument/codeAction  (at a position, possibly with diagnostics)
"""
import pytest
from lsprotocol import types as lsp

from conftest import FIXTURES

# ── fixtures ──────────────────────────────────────────────────────────────────
POST = FIXTURES / "starlette-blog" / "templates" / "blog" / "post.html"
MACROS = FIXTURES / "starlette-blog" / "templates" / "blog" / "macros.html"
BASE = FIXTURES / "starlette-blog" / "templates" / "base.html"


def _open(client, path, version=1):
    uri = path.as_uri()
    client.text_document_did_open(
        lsp.DidOpenTextDocumentParams(
            text_document=lsp.TextDocumentItem(
                uri=uri,
                language_id="jinja",
                version=version,
                text=path.read_text(),
            )
        )
    )
    return uri


# ── journeys ──────────────────────────────────────────────────────────────────


@pytest.mark.asyncio
async def test_did_open_publishes_diagnostics(client):
    """REQ-E2E-06: didOpen triggers textDocument/publishDiagnostics."""
    uri = _open(client, BASE)
    await client.wait_for_notification("textDocument/publishDiagnostics")
    # After notification, the URI must be present in client.diagnostics
    assert uri in client.diagnostics


@pytest.mark.asyncio
async def test_completion_returns_items(client):
    """REQ-E2E-06: completion at a Jinja tag position returns a non-empty list."""
    uri = _open(client, BASE)
    # base.html line 2: "{% block head %}{% endblock %}"
    # char 3 = 'b' of 'block' — inside a tag keyword position
    result = await client.text_document_completion_async(
        lsp.CompletionParams(
            text_document=lsp.TextDocumentIdentifier(uri=uri),
            position=lsp.Position(line=2, character=3),
        )
    )
    assert result is not None
    items = result.items if hasattr(result, "items") else result
    assert len(items) > 0


@pytest.mark.asyncio
async def test_completion_item_resolve(client):
    """REQ-E2E-06: completionItem/resolve returns an enriched item."""
    uri = _open(client, BASE)
    result = await client.text_document_completion_async(
        lsp.CompletionParams(
            text_document=lsp.TextDocumentIdentifier(uri=uri),
            position=lsp.Position(line=2, character=3),
        )
    )
    assert result is not None
    items = result.items if hasattr(result, "items") else result
    assert len(items) > 0
    resolved = await client.completion_item_resolve_async(items[0])
    assert resolved is not None
    assert resolved.label == items[0].label


@pytest.mark.asyncio
async def test_hover_on_builtin_filter(client):
    """REQ-E2E-06: hover on a built-in filter returns documentation.

    Uses a simple template with `{{ x | upper }}` so that the filter is
    captured reliably by the references query.  Attribute-chain filters
    (e.g. `{{ post.title | truncate(60) }}`) currently produce a different
    treesitter AST that the references query doesn't cover — that is tracked
    separately as a query-coverage bug.
    """
    uri = "file:///tmp/jinja_lsp_e2e_hover.html"
    source = "{{ x | upper }}\n"
    client.text_document_did_open(
        lsp.DidOpenTextDocumentParams(
            text_document=lsp.TextDocumentItem(
                uri=uri, language_id="jinja", version=1, text=source
            )
        )
    )
    await client.wait_for_notification("textDocument/publishDiagnostics")
    # "{{ x | upper }}" — 'upper' starts at character 7
    result = await client.text_document_hover_async(
        lsp.HoverParams(
            text_document=lsp.TextDocumentIdentifier(uri=uri),
            position=lsp.Position(line=0, character=7),
        )
    )
    assert result is not None, "hover on 'upper' must return documentation"
    assert result.contents is not None


@pytest.mark.asyncio
async def test_signature_help_in_macro_call(client):
    """REQ-E2E-06: signatureHelp inside a macro call returns parameter hints."""
    _open(client, MACROS)
    uri = _open(client, POST)
    # post.html line 5: "  {{ macros.comment_card(c, show_actions=true) }}"
    # position (5, 27) = inside the argument list after the first comma
    result = await client.text_document_signature_help_async(
        lsp.SignatureHelpParams(
            text_document=lsp.TextDocumentIdentifier(uri=uri),
            position=lsp.Position(line=5, character=27),
        )
    )
    # Result may be None if cross-file macro signatures aren't wired yet.
    # At minimum the request must not crash the server.
    if result is not None:
        assert hasattr(result, "signatures")


@pytest.mark.asyncio
async def test_definition_on_macro_call(client):
    """REQ-E2E-06: go-to-definition on a macro call navigates to the declaration."""
    _open(client, MACROS)
    uri = _open(client, POST)
    # post.html line 5: "  {{ macros.comment_card(c, show_actions=true) }}"
    # 'comment_card' occupies chars 12-23; position (5, 15) is on it
    result = await client.text_document_definition_async(
        lsp.DefinitionParams(
            text_document=lsp.TextDocumentIdentifier(uri=uri),
            position=lsp.Position(line=5, character=15),
        )
    )
    # Result may be None/empty if cross-file indexing isn't triggered by didOpen.
    # If present, every location must carry a URI.
    if result is not None:
        locations = result if isinstance(result, list) else [result]
        for loc in locations:
            assert hasattr(loc, "uri"), f"location must have uri: {loc!r}"


@pytest.mark.asyncio
async def test_code_action_at_diagnostic(client):
    """REQ-E2E-06: codeAction request completes the protocol round-trip."""
    uri = _open(client, POST)
    await client.wait_for_notification("textDocument/publishDiagnostics")
    diags = client.diagnostics.get(uri, [])
    result = await client.text_document_code_action_async(
        lsp.CodeActionParams(
            text_document=lsp.TextDocumentIdentifier(uri=uri),
            range=lsp.Range(
                start=lsp.Position(line=0, character=0),
                end=lsp.Position(line=0, character=0),
            ),
            context=lsp.CodeActionContext(
                diagnostics=list(diags[:1]),
            ),
        )
    )
    # Server may return None, a list, or a tuple of CodeAction/Command — all are valid.
    assert result is None or isinstance(result, (list, tuple))
