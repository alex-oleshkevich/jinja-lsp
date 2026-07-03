// REQ-HL-01..04: in-file occurrence highlighting.
//
// Key invariant: `index.references` contains ONLY usage sites (reads).
// Binding sites (for-loop targets, set targets, macro params) are NOT Reference
// entries.  Therefore:
//   • cursor IS on a Reference  →  usage (Read kind)
//   • cursor is NOT on a Reference but inside Jinja  →  binding (Write kind)

use crate::{
    builtins::registry::{Category, Registry},
    workspace::{
        index::TemplateIndex,
        symbols::{ReferenceKind, Span},
    },
};

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightKind {
    Read = 2,
    Write = 3,
}

#[derive(Debug, Clone)]
pub struct DocumentHighlight {
    pub range: Span,
    pub kind: HighlightKind,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Compute in-file occurrence highlights for the symbol at (`line`, `col`).
///
/// - Definitions/bindings → `Write` kind (REQ-HL-03).
/// - Usages               → `Read` kind (REQ-HL-03).
/// - Host-owned / non-symbol positions → empty (REQ-HL-04).
pub fn document_highlight(
    source: &str,
    line: u32,
    col: u32,
    index: &TemplateIndex,
    registry: &Registry,
) -> Vec<DocumentHighlight> {
    let byte = super::line_col_to_byte(source, line, col);

    if !inside_jinja(source, byte) {
        return vec![];
    }

    let word = super::word_at_byte(source, byte);
    if word.is_empty() {
        return vec![];
    }

    // Macro definition opening tag → Write at name, Read at every call site.
    if let Some(m) = index
        .macros
        .iter()
        .find(|m| m.name == word && super::byte_in_span(byte, &m.span))
    {
        let write = name_span_in_source(source, m.span.start_byte, &m.name);
        return with_write_and_reads(write, word, index);
    }

    // Block definition opening tag → Write at name.
    if let Some(b) = index
        .blocks
        .iter()
        .find(|b| b.name == word && super::byte_in_span(byte, &b.span))
    {
        let write = name_span_in_source(source, b.span.start_byte, &b.name);
        return with_write_and_reads(write, word, index);
    }

    // Host-owned (built-in) → empty (REQ-HL-04).
    if is_host_owned(word, registry) {
        return vec![];
    }

    // Determine whether cursor is on a Reference (usage) or a binding.
    let at_ref = index
        .references
        .iter()
        .any(|r| super::byte_in_span(byte, &r.span) && r.name == word);

    if !at_ref {
        // Cursor is on a binding site — the word span IS the Write range.
        let write = word_span_at(source, byte);

        if index.variables.iter().any(|v| v.name == word)
            || index.import_aliases.iter().any(|a| a.alias == word)
        {
            return with_write_and_reads(write, word, index);
        }
        return vec![];
    }

    // Cursor is on a usage. Find the binding elsewhere.

    // Locally-defined macro?
    if let Some(m) = index.macros.iter().find(|m| m.name == word) {
        let write = name_span_in_source(source, m.span.start_byte, &m.name);
        return with_write_and_reads(write, word, index);
    }

    // Locally-defined block?
    if let Some(b) = index.blocks.iter().find(|b| b.name == word) {
        let write = name_span_in_source(source, b.span.start_byte, &b.name);
        return with_write_and_reads(write, word, index);
    }

    // Variable defined in this file? jinja-lsp-kj7z: pick the binding whose
    // valid_range actually contains the cursor (tightest match, mirroring
    // rename.rs's tightest_scope_for) instead of text-scanning for the first
    // same-named for/set target in the whole file — with two loops
    // (`{% for x in a %}...{% for x in b %}`), the text scan always resolved to
    // the first loop's target regardless of which one the cursor is inside.
    if index.variables.iter().any(|v| v.name == word) {
        let write = index
            .variables
            .iter()
            .filter(|v| {
                v.name == word && v.valid_range.start_byte <= byte && byte <= v.valid_range.end_byte
            })
            .min_by_key(|v| {
                v.valid_range
                    .end_byte
                    .saturating_sub(v.valid_range.start_byte)
            })
            .map(|v| v.span.clone());
        return match write {
            Some(w) => with_write_and_reads(w, word, index),
            None => reads_only(word, index),
        };
    }

    // Import alias (cursor on usage, e.g. `m.greet` where `m` is the alias)?
    if let Some(alias) = index.import_aliases.iter().find(|a| a.alias == word) {
        let mut result = vec![DocumentHighlight {
            range: alias.span.clone(),
            kind: HighlightKind::Write,
        }];
        result.extend(reads_only(word, index));
        return result;
    }

    // From-imported name (defined in another file) → reads only (REQ-HL-03, REQ-HL-02).
    if index
        .from_imports
        .iter()
        .flat_map(|fi| &fi.names)
        .any(|n| n.name == word)
    {
        return reads_only(word, index);
    }

    // No template-owned binding found → host-owned or unknown → empty (REQ-HL-04).
    vec![]
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn with_write_and_reads(write: Span, name: &str, index: &TemplateIndex) -> Vec<DocumentHighlight> {
    let mut result = vec![];
    if write.start_byte < write.end_byte {
        result.push(DocumentHighlight {
            range: write,
            kind: HighlightKind::Write,
        });
    }
    result.extend(reads_only(name, index));
    result
}

fn reads_only(name: &str, index: &TemplateIndex) -> Vec<DocumentHighlight> {
    index
        .references
        .iter()
        .filter(|r| r.name == name && is_read_kind(r.kind))
        .map(|r| DocumentHighlight {
            range: r.span.clone(),
            kind: HighlightKind::Read,
        })
        .collect()
}

fn is_read_kind(kind: ReferenceKind) -> bool {
    // Attribute references are for member accesses (`.title`), not the head identifier.
    // We only highlight head identifier reads (Identifier/Function kinds).
    matches!(kind, ReferenceKind::Identifier | ReferenceKind::Function)
}

fn is_host_owned(name: &str, registry: &Registry) -> bool {
    [
        Category::Filter,
        Category::Function,
        Category::Test,
        Category::Variable,
        Category::ContextVariable,
    ]
    .iter()
    .any(|&cat| registry.get(cat, name).is_some())
}

/// Return the span of `name` within the tag starting at `tag_start_byte`.
fn name_span_in_source(source: &str, tag_start_byte: usize, name: &str) -> Span {
    match super::find_name_in_tag(source, tag_start_byte, name) {
        Some(abs_start) => make_span(source, abs_start, abs_start + name.len()),
        None => Span::default(),
    }
}

/// Return the word-boundary span centered at `byte`.
fn word_span_at(source: &str, byte: usize) -> Span {
    let byte = super::clamp_to_char_boundary(source, byte);
    let start = source[..byte]
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    let end = source[byte..]
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| byte + i)
        .unwrap_or(source.len());
    make_span(source, start, end)
}

fn make_span(source: &str, start: usize, end: usize) -> Span {
    let (sl, sc) = super::byte_to_line_col(source, start);
    let (el, ec) = super::byte_to_line_col(source, end);
    Span {
        start_byte: start,
        end_byte: end,
        start_line: sl,
        start_col: sc,
        end_line: el,
        end_col: ec,
    }
}

fn inside_jinja(source: &str, byte: usize) -> bool {
    super::inside_jinja(source, byte)
}
