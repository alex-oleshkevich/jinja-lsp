"""REQ-E2E-06: protocol journeys for the features test_journeys.py does not reach.

test_journeys.py covers the lifecycle plus completion/hover/signatureHelp/
definition/codeAction. This module covers the remaining nine implemented
features over the wire:

  F09 references          F13 semantic tokens     F16 call hierarchy
  F10 symbols             F14 inlay hints         F18 formatting
  F11 document highlight  F15 code lens
  F12 folding range

Why over the wire and not in the Rust suite: every one of these already has
in-process tests that call the handler directly, so what is *not* covered there
is the protocol boundary — capability advertisement, param decoding, and
serialization of the response. Both protocol bugs this project shipped lived
exactly there (the code lens with an empty `Command.command`, and the Zed
`language_ids` mapping that made the server reject didOpen); each handler was
correct and each Rust test stayed green.
"""
import pytest
from lsprotocol import types as lsp

from conftest import FIXTURES, open_doc, open_source

BLOG = FIXTURES / "starlette-blog"
POST = BLOG / "templates" / "blog" / "post.html"
MACROS = BLOG / "templates" / "blog" / "macros.html"
BASE = BLOG / "templates" / "base.html"

# macros.html, 0-based:
#   1: {% macro post_url(post) %}{{ post.slug }}{% endmacro %}
#   3:   <div>{{ comment.body }}</div>
POST_URL_DEF = lsp.Position(line=1, character=10)      # on `post_url`


def _doc(uri):
    return lsp.TextDocumentIdentifier(uri=uri)


def _whole_file(text: str) -> lsp.Range:
    lines = text.split("\n")
    return lsp.Range(
        start=lsp.Position(line=0, character=0),
        end=lsp.Position(line=len(lines) - 1, character=len(lines[-1])),
    )


async def _open_blog(client):
    """Open the whole blog workspace and await each file's first publish."""
    for path in (BASE, MACROS, POST):
        open_doc(client, path)
        await client.wait_for_notification("textDocument/publishDiagnostics")


# ── F09: find references ──────────────────────────────────────────────────────


@pytest.mark.asyncio
async def test_references_on_macro_spans_workspace(blog_client):
    """REQ-REF-02: references on a macro definition reach its cross-file call sites."""
    await _open_blog(blog_client)
    result = await blog_client.text_document_references_async(
        lsp.ReferenceParams(
            text_document=_doc(MACROS.as_uri()),
            position=POST_URL_DEF,
            context=lsp.ReferenceContext(include_declaration=True),
        )
    )
    assert result, "references on `post_url` must not be empty"
    uris = {loc.uri for loc in result}
    assert POST.as_uri() in uris, (
        f"post.html calls macros.post_url() on line 7, so it must appear in the "
        f"reference set; got {uris}"
    )


# ── F10: document & workspace symbols ─────────────────────────────────────────


@pytest.mark.asyncio
async def test_document_symbol_lists_macros(client):
    """REQ-SYM-01: documentSymbol returns this template's macros."""
    uri = open_doc(client, MACROS)
    await client.wait_for_notification("textDocument/publishDiagnostics")
    result = await client.text_document_document_symbol_async(
        lsp.DocumentSymbolParams(text_document=_doc(uri))
    )
    assert result, "documentSymbol on macros.html must not be empty"
    names = {s.name for s in result}
    assert {"post_url", "comment_card"} <= names, f"expected both macros, got {names}"


@pytest.mark.asyncio
async def test_workspace_symbol_finds_macro_by_query(blog_client):
    """REQ-SYM-04: workspace/symbol resolves a macro by name across the workspace."""
    await _open_blog(blog_client)
    result = await blog_client.workspace_symbol_async(
        lsp.WorkspaceSymbolParams(query="post_url")
    )
    assert result, "workspace/symbol query 'post_url' must not be empty"
    assert any(s.name == "post_url" for s in result), (
        f"expected a `post_url` symbol, got {[s.name for s in result]}"
    )


# ── F11: document highlight ───────────────────────────────────────────────────


@pytest.mark.asyncio
async def test_document_highlight_marks_loop_binding_and_use(client):
    """REQ-HL-01/03: a `{% for %}` target highlights as Write, its uses as Read."""
    uri = open_source(
        client,
        "file:///tmp/jinja_lsp_e2e_hl_loop.html",
        "{% for post in posts %}{{ post }}{% endfor %}\n",
    )
    await client.wait_for_notification("textDocument/publishDiagnostics")
    result = await client.text_document_document_highlight_async(
        # character 26 is inside the `post` of `{{ post }}`
        lsp.DocumentHighlightParams(
            text_document=_doc(uri), position=lsp.Position(line=0, character=26)
        )
    )
    assert result, "highlight on the `post` loop variable must not be empty"
    kinds = {h.kind for h in result}
    assert lsp.DocumentHighlightKind.Write in kinds, (
        f"the `for` target is the binding, so it must be Write; got {kinds}"
    )
    assert lsp.DocumentHighlightKind.Read in kinds, (
        f"the `{{{{ post }}}}` use must be Read; got {kinds}"
    )


@pytest.mark.xfail(
    strict=True,
    reason="REQ-HL-01 lists a macro parameter as highlightable and the F11 test "
           "plan specifies this exact doc as row 3, but that row was never "
           "implemented and document_highlight returns nothing — jinja-lsp-rwog",
)
@pytest.mark.asyncio
async def test_document_highlight_marks_macro_parameter(client):
    """REQ-HL-01 (F11 test plan row 3): a macro parameter highlights in its body."""
    uri = open_source(
        client,
        "file:///tmp/jinja_lsp_e2e_hl_param.html",
        "{% macro m(words) %}{{ words }}{% endmacro %}\n",
    )
    await client.wait_for_notification("textDocument/publishDiagnostics")
    result = await client.text_document_document_highlight_async(
        # character 24 is inside the `words` of `{{ words }}`
        lsp.DocumentHighlightParams(
            text_document=_doc(uri), position=lsp.Position(line=0, character=24)
        )
    )
    assert result, "highlight on the `words` macro parameter must not be empty"


@pytest.mark.asyncio
async def test_document_highlight_silent_on_html(client):
    """REQ-HL-04: the companion principle — no highlights outside Jinja constructs."""
    uri = open_doc(client, MACROS)
    await client.wait_for_notification("textDocument/publishDiagnostics")
    result = await client.text_document_document_highlight_async(
        # line 3 is `  <div>{{ comment.body }}</div>`; character 3 is inside `<div>`
        lsp.DocumentHighlightParams(
            text_document=_doc(uri), position=lsp.Position(line=3, character=3)
        )
    )
    assert not result, f"HTML is host-owned; must stay silent, got {result}"


# ── F12: folding range ────────────────────────────────────────────────────────


@pytest.mark.asyncio
async def test_folding_range_covers_nested_blocks(client):
    """REQ-FOLD-01: nested {% block %} regions are foldable."""
    uri = open_doc(client, BASE)
    await client.wait_for_notification("textDocument/publishDiagnostics")
    result = await client.text_document_folding_range_async(
        lsp.FoldingRangeParams(text_document=_doc(uri))
    )
    assert result, "base.html has nested blocks, so folding must not be empty"
    # `{% block body %}` (line 4) .. `{% endblock %}` (line 7) is the outer region.
    assert any(r.start_line == 4 and r.end_line >= 6 for r in result), (
        f"expected a fold for the body block spanning lines 4..7, got "
        f"{[(r.start_line, r.end_line) for r in result]}"
    )
    for r in result:
        assert r.end_line > r.start_line, f"degenerate fold range: {r}"


# ── F13: semantic tokens ──────────────────────────────────────────────────────


@pytest.mark.asyncio
async def test_semantic_tokens_full_is_well_formed(client):
    """REQ-SEM-01: semanticTokens/full returns a valid 5-tuple encoded stream."""
    uri = open_doc(client, POST)
    await client.wait_for_notification("textDocument/publishDiagnostics")
    result = await client.text_document_semantic_tokens_full_async(
        lsp.SemanticTokensParams(text_document=_doc(uri))
    )
    assert result is not None and result.data, "semantic tokens must not be empty"
    assert len(result.data) % 5 == 0, (
        f"the token stream is 5 ints per token; got {len(result.data)}"
    )
    legend = client.server_capabilities.semantic_tokens_provider.legend
    type_indices = result.data[3::5]
    assert max(type_indices) < len(legend.token_types), (
        f"token type index {max(type_indices)} is outside the advertised legend "
        f"of {len(legend.token_types)} types — the client would render garbage"
    )


@pytest.mark.asyncio
async def test_semantic_tokens_range_is_well_formed(client):
    """REQ-SEM-05: the range variant returns the same encoding."""
    uri = open_doc(client, POST)
    await client.wait_for_notification("textDocument/publishDiagnostics")
    result = await client.text_document_semantic_tokens_range_async(
        lsp.SemanticTokensRangeParams(
            text_document=_doc(uri),
            range=lsp.Range(
                start=lsp.Position(line=0, character=0),
                end=lsp.Position(line=4, character=0),
            ),
        )
    )
    assert result is not None, "semanticTokens/range must return a result"
    assert len(result.data) % 5 == 0


# ── F14: inlay hints ──────────────────────────────────────────────────────────


@pytest.mark.asyncio
async def test_inlay_hints_returned_for_template(client):
    """REQ-HINT-01: inlay hints are produced for a template with blocks and calls."""
    text = POST.read_text()
    uri = open_doc(client, POST)
    await client.wait_for_notification("textDocument/publishDiagnostics")
    result = await client.text_document_inlay_hint_async(
        lsp.InlayHintParams(text_document=_doc(uri), range=_whole_file(text))
    )
    assert result, "post.html has an endblock and macro calls, so hints must exist"
    for h in result:
        assert h.label, f"an inlay hint with an empty label renders as nothing: {h}"


# ── F15: code lens ────────────────────────────────────────────────────────────


@pytest.mark.asyncio
async def test_code_lens_carries_an_invocable_command(blog_client):
    """REQ-LENS-01/04: every lens resolves to a command the editor can actually run.

    Regression guard: shipped once with `Command.command` empty, so the lens
    rendered correctly and did nothing when clicked. A handler-level test cannot
    see this — the title is populated and only the dispatch field is blank.
    """
    await _open_blog(blog_client)
    result = await blog_client.text_document_code_lens_async(
        lsp.CodeLensParams(text_document=_doc(MACROS.as_uri()))
    )
    assert result, "macros.html defines two macros, so it must carry lenses"
    for lens in result:
        resolved = lens if lens.command is not None else (
            await blog_client.code_lens_resolve_async(lens)
        )
        assert resolved.command is not None, f"lens never resolved a command: {lens}"
        assert resolved.command.command, (
            f"lens {resolved.command.title!r} has an empty `command` field — it "
            f"renders but does nothing when clicked"
        )


# ── F16: call hierarchy ───────────────────────────────────────────────────────


@pytest.mark.asyncio
async def test_call_hierarchy_prepare_and_incoming(blog_client):
    """REQ-CALL-01/02: prepare resolves a macro, and incoming calls find its callers."""
    await _open_blog(blog_client)
    items = await blog_client.text_document_prepare_call_hierarchy_async(
        lsp.CallHierarchyPrepareParams(
            text_document=_doc(MACROS.as_uri()), position=POST_URL_DEF
        )
    )
    assert items, "prepareCallHierarchy on `post_url` must return an item"
    assert items[0].name == "post_url", f"resolved the wrong symbol: {items[0].name}"

    incoming = await blog_client.call_hierarchy_incoming_calls_async(
        lsp.CallHierarchyIncomingCallsParams(item=items[0])
    )
    assert incoming, "post.html calls post_url(), so incoming calls must not be empty"
    callers = {c.from_.uri for c in incoming}
    assert POST.as_uri() in callers, f"expected post.html among callers, got {callers}"


# ── F18: formatting over LSP ──────────────────────────────────────────────────


@pytest.mark.asyncio
async def test_formatting_returns_edits_for_unformatted_source(client):
    """REQ-FMT-07: textDocument/formatting returns edits that normalize delimiters."""
    uri = open_source(
        client, "file:///tmp/jinja_lsp_e2e_format.html", "{{x|upper}}\n"
    )
    await client.wait_for_notification("textDocument/publishDiagnostics")
    result = await client.text_document_formatting_async(
        lsp.DocumentFormattingParams(
            text_document=_doc(uri),
            options=lsp.FormattingOptions(tab_size=2, insert_spaces=True),
        )
    )
    assert result, "`{{x|upper}}` is not canonically formatted, so edits must exist"
    # REQ-FMT-01: delimiter padding. Pipe padding is asserted separately below.
    assert any(e.new_text.startswith("{{ ") for e in result), (
        f"expected delimiter padding in the edit, got {[e.new_text for e in result]}"
    )


@pytest.mark.xfail(
    strict=True,
    reason="REQ-FMT-04 requires `x|e` -> `x | e`, but src/format/mod.rs hardcodes "
           "space_around_pipe: false (a field no config file can set, documented "
           "in no spec) and strips the spaces instead. The golden fixture "
           "03_pipe_spacing.expected was regenerated against the implementation, "
           "so it defends the inverted behavior — jinja-lsp-85to",
)
@pytest.mark.asyncio
async def test_formatting_pads_filter_pipes(client):
    """REQ-FMT-04: a filter pipe gets one space on each side."""
    uri = open_source(
        client, "file:///tmp/jinja_lsp_e2e_format_pipe.html", "{{x|upper}}\n"
    )
    await client.wait_for_notification("textDocument/publishDiagnostics")
    result = await client.text_document_formatting_async(
        lsp.DocumentFormattingParams(
            text_document=_doc(uri),
            options=lsp.FormattingOptions(tab_size=2, insert_spaces=True),
        )
    )
    assert result
    assert any("{{ x | upper }}" in e.new_text for e in result), (
        f"REQ-FMT-04 requires one space each side of `|`; got "
        f"{[e.new_text for e in result]}"
    )


@pytest.mark.asyncio
async def test_formatting_is_idempotent(client):
    """REQ-FMT-07: formatting its own output is a no-op (no churn on repeated saves).

    Asserted against whatever the engine considers canonical rather than a
    literal, so this stays honest regardless of how the REQ-FMT-04 pipe-spacing
    drift is resolved.
    """
    uri = open_source(
        client, "file:///tmp/jinja_lsp_e2e_format_idem.html", "{{x|upper}}\n"
    )
    await client.wait_for_notification("textDocument/publishDiagnostics")
    params = lsp.DocumentFormattingParams(
        text_document=_doc(uri),
        options=lsp.FormattingOptions(tab_size=2, insert_spaces=True),
    )
    first = await client.text_document_formatting_async(params)
    assert first, "the source is unformatted, so the first pass must edit it"

    formatted = first[0].new_text
    settled = open_source(
        client, "file:///tmp/jinja_lsp_e2e_format_idem2.html", formatted
    )
    await client.wait_for_notification("textDocument/publishDiagnostics")
    again = await client.text_document_formatting_async(
        lsp.DocumentFormattingParams(
            text_document=_doc(settled),
            options=lsp.FormattingOptions(tab_size=2, insert_spaces=True),
        )
    )
    assert not again, (
        f"formatting is not idempotent: {formatted!r} still yields "
        f"{[e.new_text for e in again]}"
    )


@pytest.mark.asyncio
async def test_range_formatting_returns_edits(client):
    """REQ-FMT-07: rangeFormatting round-trips and confines itself to the range."""
    source = "{{x|upper}}\n{{y|lower}}\n"
    uri = open_source(client, "file:///tmp/jinja_lsp_e2e_rangefmt.html", source)
    await client.wait_for_notification("textDocument/publishDiagnostics")
    result = await client.text_document_range_formatting_async(
        lsp.DocumentRangeFormattingParams(
            text_document=_doc(uri),
            range=lsp.Range(
                start=lsp.Position(line=0, character=0),
                end=lsp.Position(line=1, character=0),
            ),
            options=lsp.FormattingOptions(tab_size=2, insert_spaces=True),
        )
    )
    assert result, "the first line is unformatted, so range formatting must edit it"
    for e in result:
        assert e.range.start.line <= 1, (
            f"range formatting escaped its requested range: {e.range}"
        )
