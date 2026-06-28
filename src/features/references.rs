// REQ-REF-01..05: workspace-wide find-references for Jinja symbols.

use std::collections::HashSet;

use crate::{
    builtins::registry::{Category, Registry},
    workspace::{
        index::{TemplateIndex, WorkspaceIndex},
        symbols::{ReferenceKind, Span},
    },
};

// ── Public types ──────────────────────────────────────────────────────────────

/// A single reference location (REQ-REF-02).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReferenceLocation {
    pub path: String,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Find all references to the symbol at (`line`, `col`) in `source`.
///
/// - Macros, blocks, and import aliases → workspace-wide (REQ-REF-01).
/// - Scope-local variables → file-local within `valid_range` (REQ-REF-05).
/// - Built-ins / host-owned → empty result (REQ-REF-04).
/// - `include_declaration`: include the definition span if `true` (REQ-REF-03).
///
/// Results are deduplicated and sorted by path then position (REQ-REF-02).
#[allow(clippy::too_many_arguments)]
pub fn find_references(
    source: &str,
    line: u32,
    col: u32,
    current_path: &str,
    include_declaration: bool,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
) -> Vec<ReferenceLocation> {
    let byte = line_col_to_byte(source, line, col);

    // Identify the symbol under the cursor.
    let Some(symbol) = symbol_at(source, byte, current_path, index, registry) else {
        return vec![];
    };

    let mut results: HashSet<ReferenceLocation> = HashSet::new();

    match symbol {
        Symbol::Macro { name, def_path, def_span } => {
            // Add declaration if requested (REQ-REF-03).
            if include_declaration {
                results.insert(span_to_ref(&def_path, &def_span));
            }
            // Collect usages across the whole workspace (REQ-REF-01, REQ-REF-02).
            for (path, tmpl_idx) in &workspace.templates {
                for r in &tmpl_idx.references {
                    if r.name == name
                        && matches!(r.kind, ReferenceKind::Identifier | ReferenceKind::Function)
                    {
                        results.insert(span_to_ref(path, &r.span));
                    }
                }
            }
        }

        Symbol::ScopeLocal { name } => {
            // File-local references only (REQ-REF-05).
            for r in &index.references {
                if r.name == name
                    && matches!(r.kind, ReferenceKind::Identifier | ReferenceKind::Function)
                {
                    results.insert(span_to_ref(current_path, &r.span));
                }
            }
            // Optionally include the declaration (variable binding).
            if include_declaration {
                if let Some(var) = index.variables.iter().find(|v| v.name == name) {
                    if var.span.start_byte < var.span.end_byte {
                        results.insert(span_to_ref(current_path, &var.span));
                    }
                }
            }
        }

        Symbol::HostOwned => return vec![],
    }

    // Sort by path then by position (REQ-REF-02).
    let mut sorted: Vec<ReferenceLocation> = results.into_iter().collect();
    sorted.sort_by(|a, b| {
        a.path.cmp(&b.path)
            .then(a.start_line.cmp(&b.start_line))
            .then(a.start_col.cmp(&b.start_col))
    });
    sorted
}

// ── Symbol identification ─────────────────────────────────────────────────────

enum Symbol {
    Macro { name: String, def_path: String, def_span: Span },
    ScopeLocal { name: String },
    HostOwned,
}

fn symbol_at(
    source: &str,
    byte: usize,
    current_path: &str,
    index: &TemplateIndex,
    registry: &Registry,
) -> Option<Symbol> {
    // Check references at cursor (span-based), picking the highest-priority kind.
    let candidate = index
        .references
        .iter()
        .filter(|r| byte_in_span(byte, &r.span))
        .max_by_key(|r| kind_priority(r.kind));

    if let Some(r) = candidate {
        return classify_reference(&r.name, current_path, index, registry);
    }

    // Check macro definition spans (cursor ON the definition itself).
    for m in &index.macros {
        if byte_in_span(byte, &m.span) {
            return Some(Symbol::Macro {
                name: m.name.clone(),
                def_path: current_path.to_owned(),
                def_span: m.span.clone(),
            });
        }
    }

    // Text-based fallback: cursor on a variable binding site (spans are zero for
    // for-loop bindings, so span-based detection silently misses them). Only
    // attempt this when we're actually inside a Jinja delimiter to avoid false
    // positives from HTML text that happens to match a variable name.
    if inside_jinja(source, byte) {
        let word = super::word_at_byte(source, byte);
        if !word.is_empty() && index.variables.iter().any(|v| v.name == word) {
            return Some(Symbol::ScopeLocal { name: word.to_owned() });
        }
    }

    None
}

fn classify_reference(
    name: &str,
    current_path: &str,
    index: &TemplateIndex,
    registry: &Registry,
) -> Option<Symbol> {
    // Macro in current template.
    if let Some(m) = index.macros.iter().find(|m| m.name == name) {
        return Some(Symbol::Macro {
            name: name.to_owned(),
            def_path: current_path.to_owned(),
            def_span: m.span.clone(),
        });
    }

    // From-imported macro.
    for fi in &index.from_imports {
        if fi.names.iter().any(|n| n.name == name) {
            return Some(Symbol::Macro {
                name: name.to_owned(),
                def_path: fi.source.clone(),
                def_span: Span::default(),
            });
        }
    }

    // Scope-local variable.
    if index.variables.iter().any(|v| v.name == name) {
        return Some(Symbol::ScopeLocal { name: name.to_owned() });
    }

    // Host-owned (built-in registry symbol) → REQ-REF-04.
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
        return Some(Symbol::HostOwned);
    }

    // Unknown identifier — treat as host-owned to avoid false positives.
    None
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn span_to_ref(path: &str, span: &Span) -> ReferenceLocation {
    ReferenceLocation {
        path: path.to_owned(),
        start_line: span.start_line,
        start_col: span.start_col,
        end_line: span.end_line,
        end_col: span.end_col,
    }
}

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

/// Returns `true` when `byte` is inside an active `{{ }}` or `{% %}` delimiter.
fn inside_jinja(source: &str, byte: usize) -> bool {
    let before = &source[..byte.min(source.len())];
    let is_active = |open: Option<usize>, close: Option<usize>| match (open, close) {
        (Some(o), Some(c)) => o > c,
        (Some(_), None) => true,
        _ => false,
    };
    let comment_active = is_active(before.rfind("{#"), before.rfind("#}"));
    if comment_active {
        return false;
    }
    is_active(before.rfind("{{"), before.rfind("}}"))
        || is_active(before.rfind("{%"), before.rfind("%}"))
}
