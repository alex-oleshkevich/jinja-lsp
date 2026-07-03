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
    let byte = super::line_col_to_byte(source, line, col);

    // ── Template reference paths (extends/include/import/from strings) ────────
    for tr in &index.template_refs {
        if super::byte_in_span(byte, &tr.span) && !tr.is_dynamic {
            if let Some(key) = workspace.resolve_key(&tr.path) {
                return Some(DefinitionLocation {
                    target_path: key.to_owned(),
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
        .filter(|r| super::byte_in_span(byte, &r.span))
        .collect();
    candidates.sort_by_key(|b| std::cmp::Reverse(super::kind_priority(b.kind)));

    for r in &candidates {
        // Filters and tests are built-ins or host-owned (REQ-DEF-06): short-circuit.
        if matches!(r.kind, ReferenceKind::Filter | ReferenceKind::Test) {
            return None;
        }
        let result = match r.kind {
            ReferenceKind::Function | ReferenceKind::Identifier => {
                resolve_ident(&r.name, byte, current_path, index, registry, workspace)
            }
            ReferenceKind::Attribute => {
                // Attribute access like `macros.post_url` — resolve via alias.
                // Special case: `self.<block>` → REQ-DEF-04 block definition.
                let parent = super::parent_of_attribute(source, r.span.start_byte);
                match parent {
                    Some("self") => resolve_self_block(&r.name, current_path, index, workspace),
                    Some(p) => resolve_alias_attr(p, &r.name, index, workspace),
                    None => None,
                }
            }
            ReferenceKind::Filter | ReferenceKind::Test => unreachable!(),
        };
        if result.is_some() {
            return result;
        }
    }

    // ── Fallback: self.<block>() not captured by references.scm ─────────────
    // When the cursor is on the method name in `self.method()`, the grammar
    // produces no Reference entry (it's an attribute+call combined form that
    // matches neither the plain-attribute nor the plain-function pattern).
    // Detect via text scan: if the word is immediately preceded by "self." →
    // treat it as a self-block reference.
    {
        let word = word_at_byte(source, byte);
        if !word.is_empty() && is_preceded_by_self_dot(source, byte) {
            return resolve_self_block(word, current_path, index, workspace);
        }
    }

    // ── From-import names (REQ-DEF-03) ───────────────────────────────────────
    // Imported names are not captured as References and fi.span only covers
    // the path string, so we match by the identifier word at the cursor.
    {
        let word = word_at_byte(source, byte);
        if !word.is_empty() {
            'outer: for fi in &index.from_imports {
                for n in &fi.names {
                    if n.name == word || n.alias.as_deref() == Some(word) {
                        if let Some(src_key) = workspace.resolve_key(&fi.source) {
                            if let Some(src_idx) = workspace.templates.get(src_key) {
                                if let Some(m) = src_idx.macros.iter().find(|m| m.name == n.name) {
                                    return Some(span_to_def(src_key, &m.span));
                                }
                            }
                        }
                        break 'outer;
                    }
                }
            }
        }
    }

    // ── Import alias spans (REQ-DEF-05) ──────────────────────────────────────
    // Cursor on the alias identifier in the import statement itself.
    for alias in &index.import_aliases {
        if super::byte_in_span(byte, &alias.span) {
            // Jump to source template (more useful than jumping to same-file declaration).
            if let Some(key) = workspace.resolve_key(&alias.source) {
                return Some(DefinitionLocation {
                    target_path: key.to_owned(),
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
        if super::byte_in_span(byte, &b.span) {
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
    reference_byte: usize,
    current_path: &str,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
) -> Option<DefinitionLocation> {
    // REQ-DEF-04: super() inside an overriding block → parent's same-named block.
    if name == "super" {
        return resolve_super(reference_byte, current_path, index, workspace);
    }
    // Macro in the current template (REQ-DEF-01).
    if let Some(m) = index.macros.iter().find(|m| m.name == name) {
        return Some(span_to_def(current_path, &m.span));
    }

    // From-imported macro (REQ-DEF-03): name was imported via `from X import Y [as Z]`.
    // When matched via alias, use the real imported name (n.name) for the macro lookup.
    'from_loop: for fi in &index.from_imports {
        for n in &fi.names {
            if n.name == name || n.alias.as_deref() == Some(name) {
                if let Some(src_key) = workspace.resolve_key(&fi.source) {
                    if let Some(src_idx) = workspace.templates.get(src_key) {
                        if let Some(m) = src_idx.macros.iter().find(|m| m.name == n.name) {
                            return Some(span_to_def(src_key, &m.span));
                        }
                    }
                }
                break 'from_loop;
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

    // REQ-DEF-08: scope-local variable (set/for/with) — jump to its binding site.
    // Pick the narrowest binding whose valid_range contains the reference position.
    let var = index.variables
        .iter()
        .filter(|v| {
            v.name == name
                && v.valid_range.start_byte < v.valid_range.end_byte
                && v.valid_range.start_byte <= reference_byte
                && reference_byte < v.valid_range.end_byte
        })
        .min_by_key(|v| v.valid_range.end_byte.saturating_sub(v.valid_range.start_byte));

    if let Some(v) = var {
        return Some(span_to_def(current_path, &v.span));
    }

    None
}

/// REQ-DEF-04: `self.<block_name>` → block declaration in current template or ancestor chain.
fn resolve_self_block(
    block_name: &str,
    current_path: &str,
    index: &TemplateIndex,
    workspace: &WorkspaceIndex,
) -> Option<DefinitionLocation> {
    if let Some(b) = index.blocks.iter().find(|b| b.name == block_name) {
        return Some(span_to_def(current_path, &b.span));
    }
    // Current template may not be in the workspace (tests pass index separately).
    // Use index.extends() to get the parent path, then walk from there.
    let parent_path = index.extends()?.path.clone();
    let chain = workspace.template_chain(&parent_path);
    for ancestor_path in &chain {
        if let Some(anc_idx) = workspace.templates.get(ancestor_path) {
            if let Some(b) = anc_idx.blocks.iter().find(|b| b.name == block_name) {
                return Some(span_to_def(ancestor_path, &b.span));
            }
        }
    }
    None
}

/// REQ-DEF-04: `super()` inside an overriding block → the parent template's same-named block.
fn resolve_super(
    reference_byte: usize,
    _current_path: &str,
    index: &TemplateIndex,
    workspace: &WorkspaceIndex,
) -> Option<DefinitionLocation> {
    let containing_block = index.blocks.iter().find(|b| {
        b.body.start_byte < b.body.end_byte
            && b.body.start_byte <= reference_byte
            && reference_byte < b.body.end_byte
    })?;
    // Use index.extends() so current template need not be in the workspace.
    let parent_path = index.extends()?.path.clone();
    let chain = workspace.template_chain(&parent_path);
    for ancestor_path in &chain {
        if let Some(anc_idx) = workspace.templates.get(ancestor_path) {
            if let Some(anc_block) = anc_idx.blocks.iter().find(|ab| ab.name == containing_block.name) {
                return Some(span_to_def(ancestor_path, &anc_block.span));
            }
        }
    }
    None
}

fn resolve_alias_attr(
    parent: &str,
    attr: &str,
    index: &TemplateIndex,
    workspace: &WorkspaceIndex,
) -> Option<DefinitionLocation> {
    let alias = index.import_aliases.iter().find(|a| a.alias == parent)?;
    let src_key = workspace.resolve_key(&alias.source)?;
    let src_idx = workspace.templates.get(src_key)?;
    let m = src_idx.macros.iter().find(|m| m.name == attr)?;
    Some(span_to_def(src_key, &m.span))
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

fn word_at_byte(source: &str, byte: usize) -> &str {
    super::word_at_byte(source, byte)
}

/// Returns true when the identifier starting at `byte` is immediately preceded by `self.`.
fn is_preceded_by_self_dot(source: &str, byte: usize) -> bool {
    source.get(..byte).is_some_and(|before| before.ends_with("self."))
}
