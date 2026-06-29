// REQ-SEM-01..06: semantic token legend, classification, and full/range requests.
//
// The token legend is the wire contract with the editor — order must never change.
// Token types and modifiers are index-encoded; the editor resolves them against the
// legend declared at `initialize`.  Index 7 of the type list is tombstoned
// (previously the retired `keyword` type) per REQ-SEM-06.
//
// Token derivation walks three sources:
//   1. index.blocks  → block-name tokens (TT_BLOCK, 0 modifiers)
//   2. index.macros  → macro definition-site tokens (TT_MACRO) + parameter tokens
//   3. index.references → reference tokens classified by kind + registry/index lookup

use crate::{
    builtins::registry::{Category, Registry, Source},
    workspace::{
        index::{TemplateIndex, WorkspaceIndex},
        symbols::ReferenceKind,
    },
};

// ── Legend (REQ-SEM-01, REQ-SEM-02, REQ-SEM-06) ──────────────────────────────

/// Token-type legend in wire-index order (REQ-SEM-01).
/// Index 7 is the tombstone slot for the retired `keyword` type — never reuse it.
pub const TOKEN_TYPES: &[&str] = &[
    "macro",     // 0
    "variable",  // 1
    "parameter", // 2
    "filter",    // 3
    "function",  // 4
    "test",      // 5
    "block",     // 6
    // 7 — tombstoned (retired keyword)
];

/// Token-modifier legend in bit-position order (REQ-SEM-02).
pub const TOKEN_MODIFIERS: &[&str] = &[
    "defined", // bit 0 (1 << 0)
    "unknown", // bit 1 (1 << 1)
    "builtin", // bit 2 (1 << 2)
    "user",    // bit 3 (1 << 3)
];

// Token type constants (match TOKEN_TYPES indices).
pub const TT_MACRO: u32 = 0;
pub const TT_VARIABLE: u32 = 1;
pub const TT_PARAMETER: u32 = 2;
pub const TT_FILTER: u32 = 3;
pub const TT_FUNCTION: u32 = 4;
pub const TT_TEST: u32 = 5;
pub const TT_BLOCK: u32 = 6;

// Modifier bit flags (match TOKEN_MODIFIERS bit positions).
pub const MOD_DEFINED: u32 = 1 << 0;
pub const MOD_UNKNOWN: u32 = 1 << 1;
pub const MOD_BUILTIN: u32 = 1 << 2;
pub const MOD_USER: u32 = 1 << 3;

// ── Public types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct SemanticToken {
    pub line: u32,
    pub start_char: u32,
    pub length: u32,
    pub token_type: u32,
    pub token_modifiers: u32,
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Classify every Jinja name in `source` into semantic tokens (REQ-SEM-03).
pub fn semantic_tokens_full(
    source: &str,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
) -> Vec<SemanticToken> {
    let mut tokens = collect_tokens(source, index, registry, workspace);
    tokens.sort_by_key(|t| (t.line, t.start_char));
    tokens
}

/// Same as `semantic_tokens_full` but restricted to lines `[start_line, end_line]` (REQ-SEM-03).
///
/// Decoded `(abs-pos, type, mods)` tuples are a subset of `full`'s for the overlapping lines.
pub fn semantic_tokens_range(
    source: &str,
    start_line: u32,
    end_line: u32,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
) -> Vec<SemanticToken> {
    semantic_tokens_full(source, index, registry, workspace)
        .into_iter()
        .filter(|t| t.line >= start_line && t.line <= end_line)
        .collect()
}

// ── Token collection ──────────────────────────────────────────────────────────

fn collect_tokens(
    source: &str,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
) -> Vec<SemanticToken> {
    let mut out = Vec::new();

    // 1. Block definition names → block token, 0 modifiers (REQ-SEM-04 §5.3.3).
    for block in &index.blocks {
        if let Some((line, col)) = find_word_in_source(source, block.span.start_byte, &block.name) {
            out.push(SemanticToken {
                line,
                start_char: col,
                length: block.name.len() as u32,
                token_type: TT_BLOCK,
                token_modifiers: 0,
            });
        }
    }

    // 2. Macro definition names → macro {defined, user}; parameters → parameter, 0 modifiers.
    for mac in &index.macros {
        // Definition-site name token.
        if let Some((line, col)) = find_word_in_source(source, mac.span.start_byte, &mac.name) {
            out.push(SemanticToken {
                line,
                start_char: col,
                length: mac.name.len() as u32,
                token_type: TT_MACRO,
                token_modifiers: MOD_DEFINED | MOD_USER,
            });
        }
        // Parameter tokens (REQ-SEM-04 §5.3.2).
        for param in &mac.parameters {
            if let Some((line, col)) =
                find_param_in_macro_tag(source, mac.span.start_byte, &mac.name, &param.name)
            {
                out.push(SemanticToken {
                    line,
                    start_char: col,
                    length: param.name.len() as u32,
                    token_type: TT_PARAMETER,
                    token_modifiers: 0,
                });
            }
        }
    }

    // 3. Reference sites → classify by kind (REQ-SEM-04).
    for r in &index.references {
        let (tt, mods) = match r.kind {
            ReferenceKind::Attribute => continue, // member accesses are not independently tokenized
            ReferenceKind::Filter => classify_filter(&r.name, registry),
            ReferenceKind::Test => classify_test(&r.name, registry),
            ReferenceKind::Function => classify_call(&r.name, index, registry, workspace),
            ReferenceKind::Identifier => classify_identifier(&r.name, index, registry),
        };
        out.push(SemanticToken {
            line: r.span.start_line,
            start_char: r.span.start_col,
            length: r.name.len() as u32,
            token_type: tt,
            token_modifiers: mods,
        });
    }

    out
}

// ── Classification helpers ────────────────────────────────────────────────────

fn classify_filter(name: &str, registry: &Registry) -> (u32, u32) {
    match registry.get(Category::Filter, name) {
        Some(entry) => (TT_FILTER, source_mods(&entry.source) | MOD_DEFINED),
        None => (TT_FILTER, MOD_UNKNOWN),
    }
}

fn classify_test(name: &str, registry: &Registry) -> (u32, u32) {
    match registry.get(Category::Test, name) {
        Some(entry) => (TT_TEST, source_mods(&entry.source) | MOD_DEFINED),
        None => (TT_TEST, MOD_UNKNOWN),
    }
}

/// Resolution order for a call site `foo(args)` — §5.3.1:
///   1. macro in index/workspace → macro {defined, user}
///   2. registry function → function {builtin/user, defined}
///   3. else → variable {unknown}  (never function {unknown})
fn classify_call(
    name: &str,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
) -> (u32, u32) {
    if is_macro(name, index, workspace) {
        return (TT_MACRO, MOD_DEFINED | MOD_USER);
    }
    if let Some(entry) = registry.get(Category::Function, name) {
        return (TT_FUNCTION, source_mods(&entry.source) | MOD_DEFINED);
    }
    (TT_VARIABLE, MOD_UNKNOWN)
}

/// Resolution for a plain identifier use (not a call):
///   1. registry variable/context-variable → variable {builtin/user, defined}
///   2. local variable binding → variable {user, defined}
///   3. import alias used as namespace → variable {user, defined}
///   4. else → variable {unknown}
fn classify_identifier(name: &str, index: &TemplateIndex, registry: &Registry) -> (u32, u32) {
    if let Some(entry) = registry.get(Category::Variable, name) {
        return (TT_VARIABLE, source_mods(&entry.source) | MOD_DEFINED);
    }
    if let Some(entry) = registry.get(Category::ContextVariable, name) {
        return (TT_VARIABLE, source_mods(&entry.source) | MOD_DEFINED);
    }
    if index.variables.iter().any(|v| v.name == name) {
        return (TT_VARIABLE, MOD_USER | MOD_DEFINED);
    }
    if index.import_aliases.iter().any(|a| a.alias == name) {
        return (TT_VARIABLE, MOD_USER | MOD_DEFINED);
    }
    (TT_VARIABLE, MOD_UNKNOWN)
}

fn is_macro(name: &str, index: &TemplateIndex, workspace: &WorkspaceIndex) -> bool {
    if index.macros.iter().any(|m| m.name == name) {
        return true;
    }
    for alias in &index.import_aliases {
        if let Some(src_idx) = workspace.templates.get(&alias.source) {
            if src_idx.macros.iter().any(|m| m.name == name) {
                return true;
            }
        }
    }
    for fi in &index.from_imports {
        if fi.names.iter().any(|n| n.name == name) {
            if let Some(src_idx) = workspace.templates.get(&fi.source) {
                if src_idx.macros.iter().any(|m| m.name == name) {
                    return true;
                }
            }
        }
    }
    false
}

fn source_mods(source: &Source) -> u32 {
    match source {
        Source::Core | Source::Pack(_) => MOD_BUILTIN,
        Source::Hint | Source::Custom => MOD_USER,
    }
}

// ── Source-text span helpers ──────────────────────────────────────────────────

/// Find `name` as a whole word in `source[start_byte..]`; return `(line, col)`.
///
/// Skips past the opening `{% keyword` so a name equal to the keyword is not matched there.
fn find_word_in_source(source: &str, start_byte: usize, name: &str) -> Option<(u32, u32)> {
    let search_from = start_byte + after_tag_keyword(source.get(start_byte..)?);
    let slice = source.get(search_from..)?;
    let name_bytes = name.as_bytes();
    let slice_bytes = slice.as_bytes();
    let mut i = 0usize;
    while i + name.len() <= slice.len() {
        if &slice_bytes[i..i + name.len()] == name_bytes {
            let before_ok = i == 0 || !is_ident(slice_bytes[i - 1]);
            let after_ok = i + name.len() >= slice.len() || !is_ident(slice_bytes[i + name.len()]);
            if before_ok && after_ok {
                return Some(byte_to_line_col(source, search_from + i));
            }
        }
        i += 1;
    }
    None
}

/// Returns the byte offset from the start of a `{% … %}` tag to just after the first keyword.
/// Input is the slice starting at `{%`. If the pattern isn't recognised, returns 0.
fn after_tag_keyword(tag: &str) -> usize {
    let inner = tag.strip_prefix("{%").unwrap_or(tag);
    let inner = inner.trim_start_matches(['-', '+', ' ', '\t']);
    let keyword_len = inner.find(|c: char| !c.is_alphanumeric() && c != '_').unwrap_or(inner.len());
    tag.len() - inner.len() + keyword_len
}

/// Find parameter `param_name` within the parentheses of `{% macro mac_name(…) %}`.
///
/// Searches only inside `(…)` to avoid matching the macro name itself or text outside the tag.
fn find_param_in_macro_tag(
    source: &str,
    macro_start: usize,
    _mac_name: &str,
    param_name: &str,
) -> Option<(u32, u32)> {
    let tag_slice = source.get(macro_start..)?;
    let paren_open = tag_slice.find('(')?;
    let paren_close = tag_slice.find(')')?;
    if paren_close <= paren_open {
        return None;
    }
    let content = &tag_slice[paren_open + 1..paren_close];
    let offset = paren_open + 1;
    let name_bytes = param_name.as_bytes();
    let content_bytes = content.as_bytes();
    let mut i = 0usize;
    while i + param_name.len() <= content.len() {
        if &content_bytes[i..i + param_name.len()] == name_bytes {
            let before_ok = i == 0 || !is_ident(content_bytes[i - 1]);
            let after_ok =
                i + param_name.len() >= content.len() || !is_ident(content_bytes[i + param_name.len()]);
            if before_ok && after_ok {
                return Some(byte_to_line_col(source, macro_start + offset + i));
            }
        }
        i += 1;
    }
    None
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn byte_to_line_col(source: &str, byte: usize) -> (u32, u32) {
    let mut line = 0u32;
    let mut col = 0u32;
    let mut pos = 0usize;
    for ch in source.chars() {
        if pos >= byte {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += ch.len_utf8() as u32;
        }
        pos += ch.len_utf8();
    }
    (line, col)
}
