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
    let Some(symbol) = symbol_at(source, byte, current_path, index, registry, workspace) else {
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
            // Skip templates that define their own local macro with this name — those calls
            // resolve to the local definition, not to the target macro in def_path.
            for (path, tmpl_idx) in &workspace.templates {
                let is_def_file = path.as_str() == def_path.as_str();
                if !is_def_file && !super::template_does_not_shadow_macro(tmpl_idx, &name) {
                    continue;
                }
                for r in &tmpl_idx.references {
                    if r.name == name
                        && matches!(r.kind, ReferenceKind::Identifier | ReferenceKind::Function)
                    {
                        results.insert(span_to_ref(path, &r.span));
                    }
                }
            }
        }

        // REQ-REF-01: blocks — collect all templates that declare a block with this name.
        // REQ-REF-03: when include_declaration=false, exclude the current-file block
        // (the definition the cursor sits on). Overrides in other files are still usages.
        Symbol::Block { name } => {
            for (path, tmpl_idx) in &workspace.templates {
                let is_current = path.as_str() == current_path;
                for b in &tmpl_idx.blocks {
                    if b.name == name && (include_declaration || !is_current) {
                        results.insert(span_to_ref(path, &b.span));
                    }
                }
            }
        }

        // REQ-REF-01: import alias — collect all identifier references to the alias name
        // across the workspace (the alias is file-local, so only current file matters).
        Symbol::Alias { name } => {
            // Locate the alias identifier's byte offset so the text scan can skip it
            // when include_declaration=false (REQ-REF-03: the scan otherwise finds the
            // alias token inside `{% import … as name %}` and leaks the declaration).
            let alias_entry = index.import_aliases.iter().find(|a| a.alias == name);
            let decl_byte = alias_entry.map(|a| a.alias_span.start_byte);
            if include_declaration {
                if let Some(alias) = alias_entry {
                    results.insert(span_to_ref(current_path, &alias.span));
                }
            }
            for r in &index.references {
                if r.name == name && r.kind == ReferenceKind::Identifier {
                    results.insert(span_to_ref(current_path, &r.span));
                }
            }
            // Text-scan for alias used in attribute position (e.g. `{{ alias.fn() }}`).
            // The @object capture may not be extracted as a reference by the extractor when
            // the grammar structure differs from the query pattern, so scan the source directly.
            let name_bytes = name.as_bytes();
            let src_bytes = source.as_bytes();
            let mut pos = 0usize;
            while pos + name_bytes.len() <= src_bytes.len() {
                if &src_bytes[pos..pos + name_bytes.len()] == name_bytes {
                    let before_ok = pos == 0 || !(src_bytes[pos - 1].is_ascii_alphanumeric() || src_bytes[pos - 1] == b'_');
                    let after = pos + name_bytes.len();
                    let after_ok = after >= src_bytes.len() || !(src_bytes[after].is_ascii_alphanumeric() || src_bytes[after] == b'_');
                    // Skip the declaration site when include_declaration=false (REQ-REF-03).
                    let is_decl = decl_byte == Some(pos);
                    if before_ok && after_ok && inside_jinja(source, pos) && (include_declaration || !is_decl) {
                        let (sl, sc) = byte_to_line_col(source, pos);
                        let (el, ec) = byte_to_line_col(source, after);
                        results.insert(ReferenceLocation { path: current_path.to_owned(), start_line: sl, start_col: sc, end_line: el, end_col: ec });
                    }
                }
                pos += 1;
            }
        }

        Symbol::ScopeLocal { name, scope } => {
            // File-local references, scoped to the binding's valid_range (REQ-REF-05):
            // two unrelated bindings sharing a name (e.g. two separate for-loops) must
            // not be merged into one reference set.
            let in_scope = |span: &Span| match &scope {
                None => true,
                Some(vr) => span.start_byte >= vr.start_byte && span.start_byte <= vr.end_byte,
            };
            for r in &index.references {
                if r.name == name
                    && matches!(r.kind, ReferenceKind::Identifier | ReferenceKind::Function)
                    && in_scope(&r.span)
                {
                    results.insert(span_to_ref(current_path, &r.span));
                }
            }
            // Optionally include the declaration (variable binding) matching this scope.
            if include_declaration {
                let var = index.variables.iter().find(|v| {
                    v.name == name
                        && scope.as_ref().is_none_or(|vr| v.valid_range == *vr)
                });
                if let Some(var) = var {
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
    Block { name: String },
    Alias { name: String },
    ScopeLocal { name: String, scope: Option<Span> },
    HostOwned,
}

fn symbol_at(
    source: &str,
    byte: usize,
    current_path: &str,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
) -> Option<Symbol> {
    // Check references at cursor (span-based), picking the highest-priority kind.
    let candidate = index
        .references
        .iter()
        .filter(|r| byte_in_span(byte, &r.span))
        .max_by_key(|r| kind_priority(r.kind));

    if let Some(r) = candidate {
        return classify_reference(&r.name, byte, current_path, index, registry, workspace);
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

    // Check block definition spans.
    for b in &index.blocks {
        if byte_in_span(byte, &b.span) {
            return Some(Symbol::Block { name: b.name.clone() });
        }
    }

    // Check import alias spans (cursor ON the alias declaration).
    for a in &index.import_aliases {
        if byte_in_span(byte, &a.span) {
            return Some(Symbol::Alias { name: a.alias.clone() });
        }
    }

    // Text-based fallback: cursor on a variable binding site (spans are zero for
    // for-loop bindings, so span-based detection silently misses them). Only
    // attempt this when we're actually inside a Jinja delimiter to avoid false
    // positives from HTML text that happens to match a variable name.
    if inside_jinja(source, byte) {
        let word = super::word_at_byte(source, byte);
        if !word.is_empty() {
            // Import alias used as namespace (e.g. `macros` in `{{ macros.fn() }}`).
            if index.import_aliases.iter().any(|a| a.alias == word) {
                return Some(Symbol::Alias { name: word.to_owned() });
            }
            if index.variables.iter().any(|v| v.name == word) {
                return Some(Symbol::ScopeLocal {
                    name: word.to_owned(),
                    scope: tightest_binding_scope(word, byte, index),
                });
            }
        }
    }

    None
}

/// Find the narrowest (smallest valid_range) VariableDefinition binding
/// whose name == `name` and whose valid_range contains `byte`.
fn tightest_binding_scope(name: &str, byte: usize, index: &TemplateIndex) -> Option<Span> {
    index.variables.iter()
        .filter(|v| {
            v.name == name
                && v.valid_range.start_byte < v.valid_range.end_byte
                && v.valid_range.start_byte <= byte
                && byte <= v.valid_range.end_byte
        })
        .min_by_key(|v| v.valid_range.end_byte.saturating_sub(v.valid_range.start_byte))
        .map(|v| v.valid_range.clone())
}

fn classify_reference(
    name: &str,
    byte: usize,
    current_path: &str,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
) -> Option<Symbol> {
    // Macro in current template.
    if let Some(m) = index.macros.iter().find(|m| m.name == name) {
        return Some(Symbol::Macro {
            name: name.to_owned(),
            def_path: current_path.to_owned(),
            def_span: m.span.clone(),
        });
    }

    // From-imported macro (including `import foo as bar` aliases).
    for fi in &index.from_imports {
        if let Some(imported) = fi.names.iter().find(|n| n.name == name || n.alias.as_deref() == Some(name)) {
            // Resolve fi.source (always relative) against the workspace's absolute keys.
            let original_name = &imported.name;
            let src_key = workspace.resolve_key(&fi.source).unwrap_or(&fi.source);
            let def_span = workspace
                .templates
                .get(src_key)
                .and_then(|tmpl| tmpl.macros.iter().find(|m| &m.name == original_name))
                .map(|m| m.span.clone())
                .unwrap_or_default();
            return Some(Symbol::Macro {
                name: name.to_owned(),
                def_path: src_key.to_owned(),
                def_span,
            });
        }
    }

    // Import alias (namespace usage like {{ macros.post_url() }}).
    if index.import_aliases.iter().any(|a| a.alias == name) {
        return Some(Symbol::Alias { name: name.to_owned() });
    }

    // Scope-local variable.
    if index.variables.iter().any(|v| v.name == name) {
        return Some(Symbol::ScopeLocal {
            name: name.to_owned(),
            scope: tightest_binding_scope(name, byte, index),
        });
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

fn byte_to_line_col(source: &str, byte: usize) -> (u32, u32) {
    let byte = byte.min(source.len());
    let (mut line, mut col) = (0u32, 0u32);
    for (i, c) in source.char_indices() {
        if i >= byte { break; }
        if c == '\n' { line += 1; col = 0; } else { col += 1; }
    }
    (line, col)
}

fn inside_jinja(source: &str, byte: usize) -> bool {
    super::inside_jinja(source, byte)
}
