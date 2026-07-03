// REQ-HOV-01..14: hover documentation for Jinja symbols.

use crate::{
    builtins::registry::{AttrDoc, Category, DocEntry, Registry},
    workspace::{
        index::{TemplateIndex, WorkspaceIndex},
        symbols::{BlockDefinition, MacroDefinition, ReferenceKind, Span, VariableScope},
    },
};

/// Result of a hover request (REQ-HOV-07).
#[derive(Debug, Clone)]
pub struct HoverResult {
    /// Markdown-formatted documentation card (REQ-HOV-14).
    pub markdown: String,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// Return hover documentation for the token at (`line`, `col`) in `source`.
///
/// Returns `None` when:
/// - The cursor is outside any Jinja delimiter (REQ-HOV-08).
/// - Inside a `{# comment #}`, `{% raw %}`, or a plain string literal.
/// - The token is recognized but has no documentation.
pub fn hover(
    source: &str,
    line: u32,
    col: u32,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
) -> Option<HoverResult> {
    let byte = super::line_col_to_byte(source, line, col);

    // ── Check extracted references (filter/function/test/identifier/attribute) ──
    // When multiple references land on the same span (e.g. "truncate" is captured
    // as both Identifier and Filter), prefer the more-specific kind.
    let mut candidates: Vec<_> = index
        .references
        .iter()
        .filter(|r| super::byte_in_span(byte, &r.span))
        .collect();

    candidates.sort_by_key(|b| std::cmp::Reverse(super::kind_priority(b.kind)));

    for r in &candidates {
        let result = match r.kind {
            ReferenceKind::Filter => {
                let name = super::resolve_filter_alias(&r.name);
                let alias_note = if name != r.name.as_str() {
                    Some(r.name.as_str())
                } else {
                    None
                };
                registry
                    .get(Category::Filter, name)
                    .map(|e| format_registry_card_with_span(e, alias_note, &r.span))
            }
            // REQ-HOV-12: special objects (caller, super) may be captured as Function
            // but are registered as Variable — fall back to Variable when no Function entry.
            // Also fall back to Filter: `truncate(60)` is captured as Function by the
            // treesitter grammar when arguments are present, but lives in Category::Filter.
            ReferenceKind::Function => {
                let entry = registry
                    .get(Category::Function, &r.name)
                    .or_else(|| registry.get(Category::Filter, &r.name))
                    .or_else(|| registry.get(Category::Variable, &r.name));
                entry.map(|e| {
                    let mut result = format_registry_card_with_span(e, None, &r.span);
                    if let Some(note) = scope_note_for_special(&r.name, byte, index) {
                        result.markdown.push_str("\n\n");
                        result.markdown.push_str(&note);
                    }
                    result
                })
            }
            ReferenceKind::Test => {
                let name = resolve_test_alias(&r.name);
                let alias_note = if name != r.name.as_str() {
                    Some(r.name.as_str())
                } else {
                    None
                };
                registry
                    .get(Category::Test, name)
                    .map(|e| format_registry_card_with_span(e, alias_note, &r.span))
            }
            ReferenceKind::Identifier => hover_identifier(&r.name, &r.span, byte, index, registry),
            ReferenceKind::Attribute => hover_attribute(&r.name, &r.span, source, registry),
        };
        if result.is_some() {
            return result;
        }
    }

    // ── Check macro definitions ───────────────────────────────────────────────
    for m in &index.macros {
        if super::byte_in_span(byte, &m.span) {
            return Some(format_macro_card(m));
        }
    }

    // ── Check block definitions ───────────────────────────────────────────────
    for b in &index.blocks {
        if super::byte_in_span(byte, &b.span) {
            return Some(format_block_card(b, index, workspace));
        }
    }

    // ── Check template references (extends / include / import / from) ─────────
    for tr in &index.template_refs {
        if super::byte_in_span(byte, &tr.span) {
            let body = if tr.is_dynamic {
                "Computed at runtime — cannot resolve statically.".to_owned()
            } else {
                let exists = workspace.get_by_ref(&tr.path).is_some();
                let note = if tr.ignore_missing && !exists {
                    " (not found — ignored)"
                } else if exists {
                    " (exists)"
                } else {
                    " (not found)"
                };
                format!("→ `{}`{}", tr.path, note)
            };
            let result = HoverResult {
                markdown: compose_card(&tr.path, "template path", None, Some(&body), None),
                start_line: tr.span.start_line,
                start_col: tr.span.start_col,
                end_line: tr.span.end_line,
                end_col: tr.span.end_col,
            };
            return Some(result);
        }
    }

    // ── Word-level fallback: check text at cursor for special objects,
    //    imported names, keyword args, and statement keywords ──────────────────
    // REQ-HOV-08: suppress hover inside {% raw %} bodies, plain string literals,
    // and outside Jinja delimiters entirely (e.g. plain HTML text that happens to
    // match a keyword, macro name, or imported name/alias).
    if is_in_raw_body(source, byte)
        || is_in_string_literal(source, byte)
        || !super::inside_jinja(source, byte)
    {
        return None;
    }
    if let Some((word, wb_start, wb_end)) = word_at_byte_range(source, byte) {
        // REQ-HOV-12: special objects not captured as references (e.g. loop as
        // an attribute parent) — look them up in the variable registry.
        if matches!(
            word,
            "loop" | "self" | "super" | "caller" | "varargs" | "kwargs"
        ) {
            let span = byte_range_to_span(source, wb_start, wb_end);
            let entry = registry
                .get(Category::Variable, word)
                .or_else(|| registry.get(Category::Function, word));
            if let Some(e) = entry {
                let mut result = format_registry_card_with_span(e, None, &span);
                if let Some(note) = scope_note_for_special(word, byte, index) {
                    result.markdown.push_str("\n\n");
                    result.markdown.push_str(&note);
                }
                return Some(result);
            }
        }

        // Local macro call-site hover: cursor on a Function reference to a macro
        // that isn't in the registry (local or workspace macro).
        if let Some(m) = index.macros.iter().find(|m| m.name == word) {
            return Some(format_macro_card(m));
        }
        if let Some(m) = workspace
            .templates
            .values()
            .flat_map(|ti| &ti.macros)
            .find(|m| m.name == word)
        {
            return Some(format_macro_card(m));
        }

        // REQ-HOV-10: from-import names and aliases.
        for fi in &index.from_imports {
            for imported in &fi.names {
                if imported.name == word {
                    let span = byte_range_to_span(source, wb_start, wb_end);
                    let macro_card = workspace
                        .get_by_ref(&fi.source)
                        .and_then(|ti| ti.macros.iter().find(|m| m.name == word))
                        .map(format_macro_card);
                    if let Some(r) = macro_card {
                        return Some(r);
                    }
                    let body = format!("Imported from `{}`", fi.source);
                    return Some(hover_result_for_span(
                        compose_card(word, "imported name", None, Some(&body), None),
                        &span,
                    ));
                }
                if imported.alias.as_deref() == Some(word) {
                    let span = byte_range_to_span(source, wb_start, wb_end);
                    let body = format!(
                        "`{word}` — alias of `{}`, imported from `{}`",
                        imported.name, fi.source
                    );
                    return Some(hover_result_for_span(
                        compose_card(word, "import alias", None, Some(&body), None),
                        &span,
                    ));
                }
            }
        }

        // REQ-HOV-10: namespace import aliases ({% import "x" as m %}).
        for ia in &index.import_aliases {
            if ia.alias == word {
                let span = byte_range_to_span(source, wb_start, wb_end);
                let body = format!("Namespace import from `{}`", ia.source);
                return Some(hover_result_for_span(
                    compose_card(word, "import alias", None, Some(&body), None),
                    &span,
                ));
            }
        }

        // REQ-HOV-11: keyword-argument names — word immediately followed by `=`.
        if is_keyword_arg_name(source, wb_end) {
            let span = byte_range_to_span(source, wb_start, wb_end);
            if let Some(result) = hover_keyword_arg(word, &span, source, wb_start, index, registry)
            {
                return Some(result);
            }
        }

        // REQ-HOV-13: statement keywords inside {% %} tags.
        if is_inside_statement_tag(source, byte) {
            if let Some(desc) = TAG_DOCS.iter().find(|(kw, _)| *kw == word) {
                let span = byte_range_to_span(source, wb_start, wb_end);
                return Some(hover_result_for_span(
                    compose_card(word, "statement keyword", None, Some(desc.1), None),
                    &span,
                ));
            }
        }
    }

    // Nothing recognized at this position — outside Jinja or in whitespace.
    None
}

// ── Hover helpers ─────────────────────────────────────────────────────────────

fn hover_identifier(
    name: &str,
    span: &Span,
    byte: usize,
    index: &TemplateIndex,
    registry: &Registry,
) -> Option<HoverResult> {
    // Check if it's a hinted context variable.
    if let Some(entry) = registry.get(Category::ContextVariable, name) {
        return Some(format_registry_card_with_span(entry, None, span));
    }

    // Check scope-locals from the extracted variables.
    if let Some(var) = index.variables.iter().find(|v| v.name == name) {
        let scope_label = scope_label(var.scope);
        let body = format!("Variable bound by a `{scope_label}` construct.");
        let md = compose_card(name, "variable", None, Some(&body), None);
        return Some(HoverResult {
            markdown: md,
            start_line: span.start_line,
            start_col: span.start_col,
            end_line: span.end_line,
            end_col: span.end_col,
        });
    }

    // Check registry variables (loop, self, etc.) — REQ-HOV-12: append scope note.
    if let Some(entry) = registry.get(Category::Variable, name) {
        let mut result = format_registry_card_with_span(entry, None, span);
        if let Some(note) = scope_note_for_special(name, byte, index) {
            result.markdown.push_str("\n\n");
            result.markdown.push_str(&note);
        }
        return Some(result);
    }

    // Unknown identifier — no documentation (REQ-HOV-08).
    None
}

fn hover_attribute(
    attr: &str,
    span: &Span,
    source: &str,
    registry: &Registry,
) -> Option<HoverResult> {
    // Determine parent by scanning backwards from the attribute start byte.
    let parent = super::parent_of_attribute(source, span.start_byte)?;
    let attr_doc = registry.get_attr(parent, attr)?;
    Some(format_attr_card(attr_doc, span))
}

fn format_registry_card_with_span(
    entry: &DocEntry,
    alias_of: Option<&str>,
    span: &Span,
) -> HoverResult {
    let kind_label = match entry.category {
        Category::Filter => "filter",
        Category::Function => "function",
        Category::Test => "test",
        Category::Variable => "variable",
        Category::ContextVariable => "context variable",
    };

    let alias_suffix = alias_of
        .map(|a| format!(" *(alias of `{a}`)*"))
        .unwrap_or_default();
    let since_suffix = entry
        .since
        .as_deref()
        .map(|s| format!(" · since {s}"))
        .unwrap_or_default();
    let heading = format!(
        "**{}** — {}{}{alias_suffix}",
        entry.name, kind_label, since_suffix
    );

    let mut parts = vec![heading];
    if let Some(sig) = &entry.signature {
        parts.push(format!("```jinja\n{sig}\n```"));
    }
    if !entry.body.trim().is_empty() {
        parts.push(entry.body.trim().to_owned());
    }

    HoverResult {
        markdown: parts.join("\n\n"),
        start_line: span.start_line,
        start_col: span.start_col,
        end_line: span.end_line,
        end_col: span.end_col,
    }
}

/// REQ-HOV-09: block inheritance card (modifiers, parent override, overriding children).
fn format_block_card(
    b: &BlockDefinition,
    index: &TemplateIndex,
    workspace: &WorkspaceIndex,
) -> HoverResult {
    let mut modifiers = Vec::new();
    if b.scoped {
        modifiers.push("scoped");
    }
    if b.required {
        modifiers.push("required");
    }

    let heading = if modifiers.is_empty() {
        format!("**{}** — block", b.name)
    } else {
        format!("**{}** — block ({})", b.name, modifiers.join(", "))
    };

    let mut parts = vec![heading];

    // Parent block (what this block overrides).
    if let Some((parent_path, parent_line)) = find_parent_block(&b.name, index, workspace) {
        parts.push(format!(
            "Overrides `{}` block in `{}` (line {})",
            b.name,
            parent_path,
            parent_line + 1
        ));
    }

    // Child templates that override this block.
    let current_path = &index.path;
    if !current_path.is_empty() {
        let overriders = find_block_overriders(&b.name, current_path, workspace);
        if !overriders.is_empty() {
            let items: Vec<String> = overriders
                .iter()
                .map(|(path, line)| format!("- `{}` (line {})", path, line + 1))
                .collect();
            parts.push(format!("Overridden by:\n{}", items.join("\n")));
        }
    }

    HoverResult {
        markdown: parts.join("\n\n"),
        start_line: b.span.start_line,
        start_col: b.span.start_col,
        end_line: b.span.end_line,
        end_col: b.span.end_col,
    }
}

fn find_parent_block(
    block_name: &str,
    index: &TemplateIndex,
    workspace: &WorkspaceIndex,
) -> Option<(String, u32)> {
    let parent_path = index.extends()?.path.clone();
    let chain = workspace.template_chain(&parent_path);
    for ancestor_path in &chain {
        if let Some(anc_idx) = workspace.templates.get(ancestor_path) {
            if let Some(block) = anc_idx.blocks.iter().find(|block| block.name == block_name) {
                return Some((ancestor_path.clone(), block.span.start_line));
            }
        }
    }
    None
}

fn find_block_overriders(
    block_name: &str,
    current_path: &str,
    workspace: &WorkspaceIndex,
) -> Vec<(String, u32)> {
    let mut result = Vec::new();
    for (path, tmpl_idx) in &workspace.templates {
        if path == current_path {
            continue;
        }
        if is_descendant_of(path, current_path, workspace) {
            if let Some(b) = tmpl_idx.blocks.iter().find(|b| b.name == block_name) {
                result.push((path.clone(), b.span.start_line));
            }
        }
    }
    result.sort();
    result
}

/// Walk the `extends()` chain of `descendant` to check if `ancestor` appears
/// (without requiring `ancestor` to be registered in the workspace).
fn is_descendant_of(descendant: &str, ancestor: &str, workspace: &WorkspaceIndex) -> bool {
    let mut current = descendant.to_owned();
    let mut seen = std::collections::HashSet::new();
    loop {
        let extends_path = match workspace.get_by_ref(&current).and_then(|idx| idx.extends()) {
            Some(e) => e.path.clone(),
            None => return false,
        };
        // Direct string match: handles the case where ancestor is not in the workspace
        // (e.g., base template passed as index only, not indexed in workspace).
        if extends_path == ancestor {
            return true;
        }
        // Resolve to canonical workspace key for abs-key workspaces.
        let parent_key = match workspace.resolve_key(&extends_path) {
            Some(k) => k.to_owned(),
            None => return false,
        };
        if parent_key == ancestor {
            return true;
        }
        if !seen.insert(parent_key.clone()) {
            return false; // cycle guard
        }
        current = parent_key;
    }
}

fn format_macro_card(m: &MacroDefinition) -> HoverResult {
    let params: Vec<String> = m
        .parameters
        .iter()
        .map(|p| {
            if let Some(d) = &p.default {
                format!("{}={}", p.name, d)
            } else {
                p.name.clone()
            }
        })
        .collect();
    let sig = format!("{}({})", m.name, params.join(", "));
    let md = compose_card(&m.name, "macro", Some(&sig), m.doc.as_deref(), None);
    HoverResult {
        markdown: md,
        start_line: m.span.start_line,
        start_col: m.span.start_col,
        end_line: m.span.end_line,
        end_col: m.span.end_col,
    }
}

fn format_attr_card(doc: &AttrDoc, span: &Span) -> HoverResult {
    let heading = if let Some(ty) = &doc.ty {
        format!("**{}** — attribute : {}", doc.attr, ty)
    } else {
        format!("**{}** — attribute", doc.attr)
    };
    HoverResult {
        markdown: heading,
        start_line: span.start_line,
        start_col: span.start_col,
        end_line: span.end_line,
        end_col: span.end_col,
    }
}

// ── Card composition (REQ-HOV-14) ─────────────────────────────────────────────

/// Compose a hover card in the fixed section order:
/// heading → signature → prose → metadata.
/// Empty sections are omitted.
fn compose_card(
    name: &str,
    kind: &str,
    signature: Option<&str>,
    body: Option<&str>,
    since: Option<&str>,
) -> String {
    let heading = if let Some(s) = since {
        format!("**{name}** — {kind} · since {s}")
    } else {
        format!("**{name}** — {kind}")
    };

    let mut parts = vec![heading];

    if let Some(sig) = signature {
        parts.push(format!("```jinja\n{sig}\n```"));
    }

    if let Some(b) = body {
        let trimmed = b.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed.to_owned());
        }
    }

    parts.join("\n\n")
}

// ── Utilities ─────────────────────────────────────────────────────────────────

/// REQ-HOV-12: return a scope-constraint note when `word` is used outside its valid scope.
fn scope_note_for_special(word: &str, byte: usize, index: &TemplateIndex) -> Option<String> {
    let in_for = index.variables.iter().any(|v| {
        v.scope == VariableScope::ForLoop
            && v.valid_range.start_byte <= byte
            && byte <= v.valid_range.end_byte
    });
    let in_block = index
        .blocks
        .iter()
        .any(|b| b.body.start_byte <= byte && byte <= b.body.end_byte);
    let in_macro = index
        .macros
        .iter()
        .any(|m| m.body.start_byte <= byte && byte <= m.body.end_byte);
    match word {
        "loop" if !in_for => Some("⚠ `loop` is only valid inside a `{% for %}` block.".to_owned()),
        "self" | "super" if !in_block => {
            Some(format!("⚠ `{word}` is only valid inside a block body."))
        }
        "caller" | "varargs" | "kwargs" if !in_macro => Some(format!(
            "⚠ `{word}` is only valid inside a macro or call block."
        )),
        _ => None,
    }
}

fn scope_label(scope: VariableScope) -> &'static str {
    match scope {
        VariableScope::Template => "set / template",
        VariableScope::Block => "block",
        VariableScope::ForLoop => "for",
        VariableScope::Macro => "macro",
        VariableScope::With => "with",
        VariableScope::CallBlock => "call",
        VariableScope::Trans => "trans",
        VariableScope::Filter => "filter",
        VariableScope::Autoescape => "autoescape",
    }
}

/// Resolve known Jinja filter aliases to their canonical name.
/// Resolve known Jinja test aliases to their canonical name.
fn resolve_test_alias(name: &str) -> &str {
    match name {
        "eq" | "==" => "equalto",
        "ne" | "!=" => "ne",
        "lt" | "<" => "lt",
        "gt" | ">" => "gt",
        "le" | "<=" => "le",
        "ge" | ">=" => "ge",
        other => other,
    }
}

/// Extract the word (identifier) at `byte` and return `(word, start_byte, end_byte)`.
fn word_at_byte_range(source: &str, byte: usize) -> Option<(&str, usize, usize)> {
    let byte = super::clamp_to_char_boundary(source, byte);
    if byte >= source.len() {
        return None;
    }
    let start = source[..byte]
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    let end_rel = source[byte..]
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(source.len() - byte);
    let end = byte + end_rel;
    let word = &source[start..end];
    if word.is_empty() {
        None
    } else {
        Some((word, start, end))
    }
}

/// Convert a byte range to a `Span` with line/col information.
fn byte_range_to_span(source: &str, start_byte: usize, end_byte: usize) -> Span {
    let (sl, sc) = super::byte_to_line_col(source, start_byte);
    let (el, ec) = super::byte_to_line_col(source, end_byte);
    Span {
        start_byte,
        end_byte,
        start_line: sl,
        start_col: sc,
        end_line: el,
        end_col: ec,
    }
}

fn hover_result_for_span(markdown: String, span: &Span) -> HoverResult {
    HoverResult {
        markdown,
        start_line: span.start_line,
        start_col: span.start_col,
        end_line: span.end_line,
        end_col: span.end_col,
    }
}

/// True when the byte at `end_byte` (first char after the word) is `=`
/// and the char after that is NOT `=` (so `==` is not a keyword arg).
fn is_keyword_arg_name(source: &str, end_byte: usize) -> bool {
    let rest = source.get(end_byte..).unwrap_or("");
    rest.starts_with('=') && !rest.starts_with("==")
}

/// REQ-HOV-08: true when `byte` is inside a `{% raw %}...{% endraw %}` body.
///
/// Reuses `find_innermost_open_block`'s stack-based tag scanning (via
/// `inside_raw_block`), which correctly treats literal Jinja tags written
/// inside a raw body (e.g. `{% raw %}{% for x in y %}...{% endraw %}`) as
/// plain text rather than real statements.
fn is_in_raw_body(source: &str, byte: usize) -> bool {
    super::completions::inside_raw_block(source, byte)
}

/// REQ-HOV-08: true when `byte` is inside a quoted string literal in the Jinja source.
/// Only considers single- and double-quoted literals that appear inside {{ }} or {% %} delimiters.
fn is_in_string_literal(source: &str, byte: usize) -> bool {
    // Walk from the nearest Jinja delimiter open to the cursor, tracking quote state.
    let byte = super::clamp_to_char_boundary(source, byte);
    let before = &source[..byte];
    let delim_start = before
        .rfind("{{")
        .or_else(|| before.rfind("{%"))
        .unwrap_or(0);
    let slice = &source[delim_start..byte];
    let mut in_single = false;
    let mut in_double = false;
    for ch in slice.chars() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            _ => {}
        }
    }
    in_single || in_double
}

fn is_inside_statement_tag(source: &str, byte: usize) -> bool {
    let byte = super::clamp_to_char_boundary(source, byte);
    if byte >= source.len() {
        return false;
    }
    let before = &source[..byte];
    let tag_start = match before.rfind("{%") {
        Some(p) => p,
        None => return false,
    };
    let after_tag_start = &source[tag_start..byte];
    !after_tag_start.contains("%}")
}

/// REQ-HOV-11: hover for a keyword-argument name.
/// Scans backward from `wb_start` to find the callee name and looks up the parameter.
fn hover_keyword_arg(
    param_name: &str,
    span: &Span,
    source: &str,
    wb_start: usize,
    index: &TemplateIndex,
    registry: &Registry,
) -> Option<HoverResult> {
    // Scan backward: skip whitespace, skip past `(`, `word(` pattern is the callee.
    let before = &source[..wb_start];
    // Strip trailing content up to the opening paren for this argument list.
    let paren_pos = before.rfind('(')?;
    let before_paren = &before[..paren_pos];
    // Extract the callee word (the identifier just before the paren).
    let callee_end = before_paren.len();
    let callee_start = before_paren
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    let callee = &before_paren[callee_start..callee_end];
    if callee.is_empty() {
        return None;
    }

    // Check if it's a macro in this template.
    if let Some(m) = index.macros.iter().find(|m| m.name == callee) {
        if let Some(p) = m.parameters.iter().find(|p| p.name == param_name) {
            let default_note = p
                .default
                .as_deref()
                .map(|d| format!(" (default: `{d}`)"))
                .unwrap_or_default();
            let body = format!("Parameter of macro `{callee}`{default_note}");
            return Some(hover_result_for_span(
                compose_card(param_name, "keyword argument", None, Some(&body), None),
                span,
            ));
        }
    }

    // Check built-in registry.
    let candidates: Vec<_> = [Category::Filter, Category::Function]
        .iter()
        .filter_map(|&cat| registry.get(cat, callee))
        .collect();
    for entry in candidates {
        if let Some(p) = entry.params.iter().find(|p| p.name == param_name) {
            let ty_note =
                p.ty.as_deref()
                    .map(|t| format!(" : `{t}`"))
                    .unwrap_or_default();
            let default_note = p
                .default
                .as_deref()
                .map(|d| format!(" (default: `{d}`)"))
                .unwrap_or_default();
            let body = format!("Parameter of `{callee}`{ty_note}{default_note}");
            return Some(hover_result_for_span(
                compose_card(param_name, "keyword argument", None, Some(&body), None),
                span,
            ));
        }
    }

    None
}

/// Embedded tag-doc set for statement keywords (REQ-HOV-13).
static TAG_DOCS: &[(&str, &str)] = &[
    (
        "for",
        "Iterates over each item in a sequence. Use `loop.index`, `loop.first`, `loop.last` to inspect the iteration state. Supports `else` for empty sequences.\n\nExample: `{% for item in items %}{{ item }}{% endfor %}`",
    ),
    ("endfor", "Closes a `{% for %}` block."),
    (
        "if",
        "Renders its body only when the condition is truthy. Chain with `{% elif %}` / `{% else %}` for alternatives.\n\nExample: `{% if user.active %}…{% endif %}`",
    ),
    (
        "elif",
        "Alternative branch in a `{% if %}` chain. Evaluated only when all prior conditions were falsy.",
    ),
    (
        "else",
        "Fallback branch for `{% if %}` and `{% for %}` (rendered when the iterable is empty).",
    ),
    ("endif", "Closes an `{% if %}` block."),
    (
        "block",
        "Defines a named section that child templates can override via `{% extends %}`. Supports `scoped` and `required` modifiers.\n\nExample: `{% block content %}default{% endblock %}`",
    ),
    ("endblock", "Closes a `{% block %}` definition."),
    (
        "macro",
        "Defines a reusable template function with positional and keyword parameters.\n\nExample: `{% macro render_post(title, body='') %}…{% endmacro %}`",
    ),
    ("endmacro", "Closes a `{% macro %}` definition."),
    (
        "set",
        "Assigns a value to a variable. Use `{% set x %}…{% endset %}` (block form) to capture a rendered string.\n\nExample: `{% set url = 'https://example.com' %}`",
    ),
    ("endset", "Closes a block-form `{% set %}`."),
    (
        "with",
        "Opens a new scope where additional variables can be defined. Variables set inside `{% with %}` are scoped to its body.\n\nExample: `{% with total = price * qty %}{{ total }}{% endwith %}`",
    ),
    ("endwith", "Closes a `{% with %}` scope."),
    (
        "extends",
        "Makes this template inherit from a parent layout. Must be the first tag in the file.\n\nExample: `{% extends 'base.html' %}`",
    ),
    (
        "include",
        "Renders another template inline, sharing the current context.\n\nExample: `{% include 'partials/nav.html' %}`",
    ),
    (
        "import",
        "Imports macros from another template into a namespace.\n\nExample: `{% import 'macros.html' as macros %}`",
    ),
    (
        "from",
        "Imports specific macros from another template.\n\nExample: `{% from 'macros.html' import render_post, render_comment %}`",
    ),
    (
        "call",
        "Invokes a macro while passing a caller block as `caller()`.\n\nExample: `{% call(p) render_dialog(p) %}content{% endcall %}`",
    ),
    ("endcall", "Closes a `{% call %}` block."),
    (
        "filter",
        "Applies a filter to the enclosed content.\n\nExample: `{% filter upper %}hello{% endfilter %}`",
    ),
    ("endfilter", "Closes a `{% filter %}` block."),
    (
        "raw",
        "Outputs its content verbatim — Jinja delimiters inside are not processed.\n\nExample: `{% raw %}{{ not_a_variable }}{% endraw %}`",
    ),
    ("endraw", "Closes a `{% raw %}` block."),
    (
        "autoescape",
        "Enables or disables HTML auto-escaping for the enclosed block.\n\nExample: `{% autoescape true %}{{ html }}{% endautoescape %}`",
    ),
    ("endautoescape", "Closes an `{% autoescape %}` block."),
    (
        "do",
        "Executes an expression for its side effects without rendering output.\n\nExample: `{% do list.append(item) %}`",
    ),
    (
        "trans",
        "Marks a string for translation (i18n).\n\nExample: `{% trans %}Hello, world!{% endtrans %}`",
    ),
    ("endtrans", "Closes a `{% trans %}` block."),
    (
        "pluralize",
        "Inside `{% trans %}`, selects singular/plural form based on a count.",
    ),
    (
        "continue",
        "Skips the rest of the current loop iteration (Jinja2 extension).",
    ),
    (
        "break",
        "Exits the current loop immediately (Jinja2 extension).",
    ),
];
