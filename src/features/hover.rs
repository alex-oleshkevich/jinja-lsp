// REQ-HOV-01..14: hover documentation for Jinja symbols.

use crate::{
    builtins::registry::{AttrDoc, Category, DocEntry, Registry},
    workspace::{
        index::{TemplateIndex, WorkspaceIndex},
        symbols::{MacroDefinition, ReferenceKind, Span, VariableScope},
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
    let byte = line_col_to_byte(source, line, col);

    // ── Check extracted references (filter/function/test/identifier/attribute) ──
    // When multiple references land on the same span (e.g. "truncate" is captured
    // as both Identifier and Filter), prefer the more-specific kind.
    let mut candidates: Vec<_> = index
        .references
        .iter()
        .filter(|r| byte_in_span(byte, &r.span))
        .collect();

    candidates.sort_by_key(|b| std::cmp::Reverse(kind_priority(b.kind)));

    for r in &candidates {
        let result = match r.kind {
            ReferenceKind::Filter => {
                let name = resolve_filter_alias(&r.name);
                let alias_note = if name != r.name.as_str() { Some(r.name.as_str()) } else { None };
                registry
                    .get(Category::Filter, name)
                    .map(|e| format_registry_card_with_span(e, alias_note, &r.span))
            }
            ReferenceKind::Function => registry
                .get(Category::Function, &r.name)
                .map(|e| format_registry_card_with_span(e, None, &r.span)),
            ReferenceKind::Test => {
                let name = resolve_test_alias(&r.name);
                let alias_note = if name != r.name.as_str() { Some(r.name.as_str()) } else { None };
                registry
                    .get(Category::Test, name)
                    .map(|e| format_registry_card_with_span(e, alias_note, &r.span))
            }
            ReferenceKind::Identifier => hover_identifier(&r.name, &r.span, index, registry),
            ReferenceKind::Attribute => hover_attribute(&r.name, &r.span, source, registry),
        };
        if result.is_some() {
            return result;
        }
    }

    if !candidates.is_empty() {
        // Recognized position but no documentation → None (REQ-HOV-08 fallback).
        return None;
    }

    // ── Check macro definitions ───────────────────────────────────────────────
    for m in &index.macros {
        if byte_in_span(byte, &m.span) {
            return Some(format_macro_card(m));
        }
    }

    // ── Check block definitions ───────────────────────────────────────────────
    for b in &index.blocks {
        if byte_in_span(byte, &b.span) {
            let result = HoverResult {
                markdown: compose_card(
                    &b.name,
                    "block",
                    None,
                    None,
                    None,
                ),
                start_line: b.span.start_line,
                start_col: b.span.start_col,
                end_line: b.span.end_line,
                end_col: b.span.end_col,
            };
            return Some(result);
        }
    }

    // ── Check template references (extends / include / import / from) ─────────
    for tr in &index.template_refs {
        if byte_in_span(byte, &tr.span) {
            let body = if tr.is_dynamic {
                "Computed at runtime — cannot resolve statically.".to_owned()
            } else {
                let exists = workspace.templates.contains_key(&tr.path);
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
                markdown: compose_card(
                    &tr.path,
                    "template path",
                    None,
                    Some(&body),
                    None,
                ),
                start_line: tr.span.start_line,
                start_col: tr.span.start_col,
                end_line: tr.span.end_line,
                end_col: tr.span.end_col,
            };
            return Some(result);
        }
    }

    // Nothing recognized at this position — outside Jinja or in whitespace.
    None
}

// ── Hover helpers ─────────────────────────────────────────────────────────────

fn hover_identifier(
    name: &str,
    span: &Span,
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

    // Check registry variables (loop, self, etc.).
    if let Some(entry) = registry.get(Category::Variable, name) {
        return Some(format_registry_card_with_span(entry, None, span));
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
    let parent = parent_of_attribute(source, span.start_byte)?;
    let attr_doc = registry.get_attr(parent, attr)?;
    Some(format_attr_card(attr_doc, span))
}

fn format_registry_card_with_span(entry: &DocEntry, alias_of: Option<&str>, span: &Span) -> HoverResult {
    let kind_label = match entry.category {
        Category::Filter => "filter",
        Category::Function => "function",
        Category::Test => "test",
        Category::Variable => "variable",
        Category::ContextVariable => "context variable",
    };

    let alias_suffix = alias_of.map(|a| format!(" *(alias of `{a}`)*")).unwrap_or_default();
    let since_suffix = entry.since.as_deref().map(|s| format!(" · since {s}")).unwrap_or_default();
    let heading = format!("**{}** — {}{}{alias_suffix}", entry.name, kind_label, since_suffix);

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
    let md = compose_card(&m.name, "macro", Some(&sig), None, None);
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

fn byte_in_span(byte: usize, span: &Span) -> bool {
    span.start_byte < span.end_byte && span.start_byte <= byte && byte < span.end_byte
}

fn line_col_to_byte(source: &str, target_line: u32, target_col: u32) -> usize {
    let mut byte = 0usize;
    for (i, line) in source.split('\n').enumerate() {
        if i == target_line as usize {
            return byte + (target_col as usize).min(line.len());
        }
        byte += line.len() + 1; // +1 for the '\n'
    }
    byte
}

fn kind_priority(kind: ReferenceKind) -> u8 {
    match kind {
        ReferenceKind::Filter => 5,
        ReferenceKind::Function => 4,
        ReferenceKind::Test => 3,
        ReferenceKind::Identifier => 2,
        ReferenceKind::Attribute => 1,
    }
}

/// Scan backwards from `attr_start_byte` in `source` to find the identifier
/// that precedes the `.` before the attribute.
fn parent_of_attribute(source: &str, attr_start_byte: usize) -> Option<&str> {
    if attr_start_byte == 0 {
        return None;
    }
    let before = source.get(..attr_start_byte)?;
    let dot_pos = before.rfind('.')?;
    let before_dot = &before[..dot_pos];
    let end = before_dot.len();
    let start = before_dot
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    let parent = &before_dot[start..end];
    if parent.is_empty() { None } else { Some(parent) }
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
fn resolve_filter_alias(name: &str) -> &str {
    match name {
        "d" => "default",
        "e" => "escape",
        "count" => "length",
        other => other,
    }
}

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
