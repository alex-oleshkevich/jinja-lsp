// REQ-DEF-01..09: go-to-definition for Jinja symbols.

use crate::{
    builtins::registry::{Category, Registry},
    workspace::{
        index::{TemplateIndex, WorkspaceIndex},
        symbols::{ReferenceKind, Span},
    },
};

// ── Public types ──────────────────────────────────────────────────────────────

/// A resolved definition location (REQ-DEF-07).
#[derive(Debug, Clone)]
pub struct DefinitionLocation {
    /// Absolute path of the file that contains the definition.
    pub target_path: String,
    pub target_start_line: u32,
    pub target_start_col: u32,
    pub target_end_line: u32,
    pub target_end_col: u32,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Resolve the definition of the symbol at (`line`, `col`) in `source`.
///
/// Returns `None` for:
/// - Built-in / host-owned symbols (REQ-DEF-06)
/// - Unresolvable identifiers
/// - Positions outside Jinja delimiters
pub fn go_to_definition(
    source: &str,
    line: u32,
    col: u32,
    current_path: &str,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
) -> Option<DefinitionLocation> {
    let byte = line_col_to_byte(source, line, col);

    // ── Template reference paths (extends/include/import/from strings) ────────
    for tr in &index.template_refs {
        if byte_in_span(byte, &tr.span) && !tr.is_dynamic {
            if workspace.templates.contains_key(&tr.path) {
                return Some(DefinitionLocation {
                    target_path: tr.path.clone(),
                    target_start_line: 0,
                    target_start_col: 0,
                    target_end_line: 0,
                    target_end_col: 0,
                });
            }
            return None; // path not in workspace (REQ-DEF-02: unknown path)
        }
    }

    // ── References: identifiers, functions, attributes ────────────────────────
    let mut candidates: Vec<_> = index
        .references
        .iter()
        .filter(|r| byte_in_span(byte, &r.span))
        .collect();
    candidates.sort_by(|a, b| kind_priority(b.kind).cmp(&kind_priority(a.kind)));

    for r in &candidates {
        let result = match r.kind {
            ReferenceKind::Function | ReferenceKind::Identifier => {
                resolve_ident(&r.name, current_path, index, registry, workspace)
            }
            ReferenceKind::Attribute => {
                // Attribute access like `macros.post_url` — resolve via alias.
                let parent = parent_of_attribute(source, r.span.start_byte);
                parent.and_then(|p| resolve_alias_attr(p, &r.name, index, workspace))
            }
            // Filters and tests are built-ins or host-owned (REQ-DEF-06).
            ReferenceKind::Filter | ReferenceKind::Test => None,
        };
        if result.is_some() {
            return result;
        }
    }

    // After checking references: if we hit any candidates (recognized but not
    // resolvable), stop — REQ-DEF-06 says return nothing.
    if !candidates.is_empty() {
        return None;
    }

    // ── From-import names (REQ-DEF-03) ───────────────────────────────────────
    // Imported names are not captured as References and fi.span only covers
    // the path string, so we match by the identifier word at the cursor.
    {
        let word = word_at_byte(source, byte);
        if !word.is_empty() {
            for fi in &index.from_imports {
                let matched = fi.names.iter().any(|n| {
                    n.name == word || n.alias.as_deref() == Some(word)
                });
                if matched {
                    if let Some(src_idx) = workspace.templates.get(&fi.source) {
                        if let Some(m) = src_idx.macros.iter().find(|m| m.name == word) {
                            return Some(span_to_def(&fi.source, &m.span));
                        }
                    }
                }
            }
        }
    }

    // ── Import alias spans (REQ-DEF-05) ──────────────────────────────────────
    // Cursor on the alias identifier in the import statement itself.
    for alias in &index.import_aliases {
        if byte_in_span(byte, &alias.span) {
            // Jump to source template (more useful than jumping to same-file declaration).
            if workspace.templates.contains_key(&alias.source) {
                return Some(DefinitionLocation {
                    target_path: alias.source.clone(),
                    target_start_line: 0,
                    target_start_col: 0,
                    target_end_line: 0,
                    target_end_col: 0,
                });
            }
        }
    }

    // ── Block names (REQ-DEF-04: child block → ancestor declaration) ──────────
    for b in &index.blocks {
        if byte_in_span(byte, &b.span) {
            // Walk the inheritance chain to find the nearest ancestor declaring this block.
            let chain = workspace.template_chain(current_path);
            for ancestor_path in chain.iter().skip(1) {
                if let Some(anc_idx) = workspace.templates.get(ancestor_path) {
                    if let Some(anc_block) = anc_idx.blocks.iter().find(|ab| ab.name == b.name) {
                        return Some(span_to_def(ancestor_path, &anc_block.span));
                    }
                }
            }
            return None; // block introduces a new name (no ancestor declares it)
        }
    }

    None
}

// ── Resolution helpers ────────────────────────────────────────────────────────

fn resolve_ident(
    name: &str,
    current_path: &str,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
) -> Option<DefinitionLocation> {
    // Macro in the current template (REQ-DEF-01).
    if let Some(m) = index.macros.iter().find(|m| m.name == name) {
        return Some(span_to_def(current_path, &m.span));
    }

    // From-imported macro (REQ-DEF-03): name was imported via `from X import Y`.
    for fi in &index.from_imports {
        let matched = fi.names.iter().any(|n| n.name == name || n.alias.as_deref() == Some(name));
        if matched {
            if let Some(src_idx) = workspace.templates.get(&fi.source) {
                if let Some(m) = src_idx.macros.iter().find(|m| m.name == name) {
                    return Some(span_to_def(&fi.source, &m.span));
                }
            }
        }
    }

    // Import alias usage (REQ-DEF-05): `macros` → jump to the alias declaration span.
    if let Some(alias) = index.import_aliases.iter().find(|a| a.alias == name) {
        return Some(span_to_def(current_path, &alias.span));
    }

    // Registry symbols are host-owned (REQ-DEF-06): return None.
    let in_registry = [
        Category::Filter,
        Category::Function,
        Category::Test,
        Category::Variable,
        Category::ContextVariable,
    ]
    .iter()
    .any(|&cat| registry.get(cat, name).is_some());

    if in_registry {
        return None;
    }

    // Free variable or unresolvable — return None.
    None
}

fn resolve_alias_attr<'a>(
    parent: &str,
    attr: &str,
    index: &TemplateIndex,
    workspace: &WorkspaceIndex,
) -> Option<DefinitionLocation> {
    let alias = index.import_aliases.iter().find(|a| a.alias == parent)?;
    let src_idx = workspace.templates.get(&alias.source)?;
    let m = src_idx.macros.iter().find(|m| m.name == attr)?;
    Some(span_to_def(&alias.source, &m.span))
}

fn span_to_def(path: &str, span: &Span) -> DefinitionLocation {
    DefinitionLocation {
        target_path: path.to_owned(),
        target_start_line: span.start_line,
        target_start_col: span.start_col,
        target_end_line: span.end_line,
        target_end_col: span.end_col,
    }
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn byte_in_span(byte: usize, span: &Span) -> bool {
    span.start_byte < span.end_byte && span.start_byte <= byte && byte < span.end_byte
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

fn line_col_to_byte(source: &str, target_line: u32, target_col: u32) -> usize {
    let mut byte = 0usize;
    for (i, line) in source.split('\n').enumerate() {
        if i == target_line as usize {
            return byte + (target_col as usize).min(line.len());
        }
        byte += line.len() + 1;
    }
    byte
}

/// Scan backwards from `attr_start_byte` to find the identifier before the `.`.
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

/// Extract the Jinja identifier word centered at `byte` in `source`.
fn word_at_byte(source: &str, byte: usize) -> &str {
    let start = source[..byte]
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    let end = source[byte..]
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| byte + i)
        .unwrap_or(source.len());
    &source[start..end]
}
