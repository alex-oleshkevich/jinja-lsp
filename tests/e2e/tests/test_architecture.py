"""E2E tests for E01 architecture requirements.

REQ-ARCH-04: debounced Pass 2 with generation guard
REQ-ARCH-05: didSave triggers relink; didClose keeps file indexed
REQ-ARCH-06: config change triggers reload+relink, not Pass 1 on TOML
REQ-ARCH-08: capabilities declared in initialize response
"""
import pytest
import pytest_lsp
from lsprotocol import types as lsp

from conftest import FIXTURES, open_doc, open_source

SYNTAX_ERR = sorted((FIXTURES / "syntax-errors" / "templates").glob("*.html"))[0]


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


# ── REQ-ARCH-08: initialize returns immediately, scan runs in the background ──


@pytest.mark.asyncio
async def test_initialize_declares_diagnostic_provider(blog_client):
    """REQ-ARCH-09: the server must advertise pull mode, not only push.

    Zed is a pull-mode client: without `diagnosticProvider` it never issues
    textDocument/diagnostic and shows no findings at all.
    """
    caps = blog_client.server_capabilities
    assert caps.diagnostic_provider is not None, (
        "REQ-ARCH-09: diagnosticProvider must be declared"
    )


@pytest.mark.asyncio
async def test_pull_diagnostics_match_the_push_payload(blog_client):
    """REQ-ARCH-09: push and pull deliver the identical, already-filtered result."""
    uri = open_doc(blog_client, SYNTAX_ERR)
    await blog_client.wait_for_notification("textDocument/publishDiagnostics")
    pushed = list(blog_client.diagnostics[uri])
    assert pushed, "the fixture must produce findings, or this proves nothing"

    report = await blog_client.text_document_diagnostic_async(
        lsp.DocumentDiagnosticParams(text_document=lsp.TextDocumentIdentifier(uri=uri))
    )
    pulled = report.items if hasattr(report, "items") else report.full_document_diagnostic_report.items

    key = lambda d: (d.range.start.line, d.range.start.character, str(d.code))
    assert sorted(map(key, pulled)) == sorted(map(key, pushed)), (
        f"push and pull must agree.\npush: {sorted(map(key, pushed))}\n"
        f"pull: {sorted(map(key, pulled))}"
    )


@pytest.mark.asyncio
async def test_noqa_suppressed_findings_are_absent_from_pull_mode(client):
    """REQ-ARCH-09 / F01 §101: suppression is applied before storing, not on publish.

    Filtering only on the way out to publish_diagnostics would let a pull-mode
    client see a finding a push-mode client never gets.
    """
    source = '{% import "nope.html" as m %}{# noqa #}\n'
    uri = open_source(client, "file:///tmp/jinja_lsp_e2e_noqa_pull.html", source)
    await client.wait_for_notification("textDocument/publishDiagnostics")
    assert not list(client.diagnostics.get(uri, [])), "push must honour the noqa"

    report = await client.text_document_diagnostic_async(
        lsp.DocumentDiagnosticParams(text_document=lsp.TextDocumentIdentifier(uri=uri))
    )
    pulled = report.items if hasattr(report, "items") else report.full_document_diagnostic_report.items
    assert not pulled, f"pull must honour the same noqa; got {pulled}"


@pytest.mark.asyncio
async def test_two_rapid_did_changes_apply_in_order(client):
    """REQ-ARCH-11: the newest edit wins; an out-of-order apply would corrupt state."""
    uri = open_source(client, "file:///tmp/jinja_lsp_e2e_order.html", "{{ a }}\n")
    await client.wait_for_notification("textDocument/publishDiagnostics")

    for version, text in ((2, "{% for x in y %}\n"), (3, "{{ final }}\n")):
        client.text_document_did_change(
            lsp.DidChangeTextDocumentParams(
                text_document=lsp.VersionedTextDocumentIdentifier(uri=uri, version=version),
                content_changes=[lsp.TextDocumentContentChangeWholeDocument(text=text)],
            )
        )
    await client.wait_for_notification("textDocument/publishDiagnostics")

    # v2 is an unclosed {% for %} and would report JINJA-E001; v3 is clean. If the
    # edits landed out of order the stale syntax error would still be showing.
    report = await client.text_document_diagnostic_async(
        lsp.DocumentDiagnosticParams(text_document=lsp.TextDocumentIdentifier(uri=uri))
    )
    pulled = report.items if hasattr(report, "items") else report.full_document_diagnostic_report.items
    codes = {str(d.code) for d in pulled}
    assert "JINJA-E001" not in codes, (
        f"the superseded v2 edit is still reflected in the index; got {codes}"
    )
