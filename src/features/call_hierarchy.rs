// F16 — Call hierarchy: prepare/incoming/outgoing for macros. REQ-CALL-01..04.

use std::collections::HashMap;

use crate::{
    builtins::registry::{Category, Registry},
    workspace::{
        index::{TemplateIndex, WorkspaceIndex},
        symbols::{MacroDefinition, ReferenceKind, Span, TemplateRefKind},
    },
};

// ── Public types ──────────────────────────────────────────────────────────────

/// LSP SymbolKind values used in call hierarchy items.
#[derive(Debug, Clone, PartialEq)]
pub enum ItemKind {
    /// Function (12) — macros and registry globals.
    Function,
    /// Module (11) — templates acting as callers or include/import edges.
    Module,
}

/// A zero-based line/col range.
#[derive(Debug, Clone, PartialEq)]
pub struct HierarchyRange {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// One node in the call hierarchy tree.
#[derive(Debug, Clone)]
pub struct CallHierarchyItem {
    pub name: String,
    pub kind: ItemKind,
    /// Template path (for macros/templates) or "global — <pack>" (for registry entries).
    pub detail: String,
    /// Template path as the document URI, or "jinja-builtin:<name>" for registry globals.
    pub uri: String,
    pub range: HierarchyRange,
    pub selection_range: HierarchyRange,
}

/// An incoming call — one caller grouped by enclosing owner (REQ-CALL-02).
#[derive(Debug, Clone)]
pub struct IncomingCall {
    pub from: CallHierarchyItem,
    pub from_ranges: Vec<HierarchyRange>,
}

/// An outgoing call — one dependency (macro call, template edge) (REQ-CALL-03).
#[derive(Debug, Clone)]
pub struct OutgoingCall {
    pub to: CallHierarchyItem,
    pub from_ranges: Vec<HierarchyRange>,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Resolve the macro under the cursor to a `CallHierarchyItem` (REQ-CALL-01).
/// Returns a one-element list on success, empty when cursor is not on a macro.
pub fn prepare_call_hierarchy(
    source: &str,
    line: u32,
    col: u32,
    path: &str,
    index: &TemplateIndex,
    workspace: &WorkspaceIndex,
    _registry: &Registry,
) -> Vec<CallHierarchyItem> {
    let byte = line_col_to_byte(source, line, col);

    // 1. Check references first (more specific — identifies which symbol at cursor).
    let ref_at = index
        .references
        .iter()
        .filter(|r| matches!(r.kind, ReferenceKind::Function | ReferenceKind::Identifier))
        .find(|r| byte_in_span(byte, &r.span));

    if let Some(r) = ref_at {
        if let Some(item) = resolve_macro_item(&r.name, path, index, workspace) {
            return vec![item];
        }
        // Found a reference but it's not a user macro (e.g. a registry global).
        return vec![];
    }

    // 2. Check macro definitions (cursor on the definition header itself).
    if let Some(m) = index.macros.iter().find(|m| byte_in_span(byte, &m.span)) {
        return vec![macro_item(m, path)];
    }

    vec![]
}

/// Return all call sites that invoke the macro, grouped by enclosing owner (REQ-CALL-02).
pub fn incoming_calls(item: &CallHierarchyItem, workspace: &WorkspaceIndex) -> Vec<IncomingCall> {
    let macro_name = &item.name;

    // Key: (template_path, Option<enclosing_macro_name>) — the enclosing owner identity.
    let mut groups: HashMap<(String, Option<String>), (CallHierarchyItem, Vec<HierarchyRange>)> =
        HashMap::new();

    for (tpl_path, tmpl_idx) in &workspace.templates {
        for r in &tmpl_idx.references {
            if r.name != *macro_name {
                continue;
            }
            if !matches!(r.kind, ReferenceKind::Function | ReferenceKind::Identifier) {
                continue;
            }

            let enclosing_name = find_enclosing_macro(tmpl_idx, &r.span).map(|m| m.name.clone());
            let key = (tpl_path.clone(), enclosing_name.clone());

            let from_range = span_to_range(&r.span);
            let entry = groups.entry(key).or_insert_with(|| {
                let owner = if let Some(ref name) = enclosing_name {
                    if let Some(m) = tmpl_idx.macros.iter().find(|m| &m.name == name) {
                        macro_item(m, tpl_path)
                    } else {
                        template_item(tpl_path)
                    }
                } else {
                    template_item(tpl_path)
                };
                (owner, vec![])
            });
            entry.1.push(from_range);
        }
    }

    groups
        .into_values()
        .map(|(from, from_ranges)| IncomingCall { from, from_ranges })
        .collect()
}

/// Return the direct dependencies of a macro's body (REQ-CALL-03).
pub fn outgoing_calls(
    item: &CallHierarchyItem,
    workspace: &WorkspaceIndex,
    registry: &Registry,
) -> Vec<OutgoingCall> {
    let Some(tmpl_idx) = workspace.templates.get(&item.uri) else {
        return vec![];
    };
    let Some(macro_def) = tmpl_idx.macros.iter().find(|m| m.name == item.name) else {
        return vec![];
    };
    let body = &macro_def.body;

    // Key: unique edge identifier (name or "__tpl__path").
    let mut edges: HashMap<String, (CallHierarchyItem, Vec<HierarchyRange>)> = HashMap::new();

    // Called macros and registry globals — Function/Identifier references inside the body.
    for r in &tmpl_idx.references {
        if !matches!(r.kind, ReferenceKind::Function | ReferenceKind::Identifier) {
            continue;
        }
        if !span_contains(body, &r.span) {
            continue;
        }

        let key = r.name.clone();
        let range = span_to_range(&r.span);

        if let Some(entry) = edges.get_mut(&key) {
            entry.1.push(range);
            continue;
        }

        let to_item = if let Some(m) = tmpl_idx.macros.iter().find(|m| m.name == r.name) {
            // Local macro.
            macro_item(m, &item.uri)
        } else if let Some((def_path, m)) = find_macro_in_workspace(workspace, &r.name) {
            // Macro defined elsewhere in the workspace.
            macro_item(&m, &def_path)
        } else if registry.get(Category::Function, &r.name).is_some() {
            // Registry global — terminal, synthetic URI.
            global_item(&r.name, "global")
        } else {
            continue; // unknown — skip
        };

        edges.insert(key, (to_item, vec![range]));
    }

    // Template edges — include/import refs inside the body (REQ-CALL-03 §5.3).
    for tref in &tmpl_idx.template_refs {
        if !matches!(tref.kind, TemplateRefKind::Include | TemplateRefKind::Import) {
            continue;
        }
        if tref.is_dynamic || tref.ignore_missing {
            continue;
        }
        if !span_contains(body, &tref.span) {
            continue;
        }

        let key = format!("__tpl__{}", tref.path);
        let range = span_to_range(&tref.span);

        if let Some(entry) = edges.get_mut(&key) {
            entry.1.push(range);
            continue;
        }

        let to_item = template_item(&tref.path);
        edges.insert(key, (to_item, vec![range]));
    }

    edges
        .into_values()
        .map(|(to, from_ranges)| OutgoingCall { to, from_ranges })
        .collect()
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn macro_item(m: &MacroDefinition, path: &str) -> CallHierarchyItem {
    CallHierarchyItem {
        name: m.name.clone(),
        kind: ItemKind::Function,
        detail: path.to_owned(),
        uri: path.to_owned(),
        range: span_to_range(&m.span),
        selection_range: HierarchyRange {
            start_line: m.span.start_line,
            start_col: m.span.start_col,
            end_line: m.span.start_line,
            end_col: m.span.start_col + m.name.len() as u32,
        },
    }
}

fn template_item(path: &str) -> CallHierarchyItem {
    let zero = HierarchyRange { start_line: 0, start_col: 0, end_line: 0, end_col: 0 };
    CallHierarchyItem {
        name: path.to_owned(),
        kind: ItemKind::Module,
        detail: path.to_owned(),
        uri: path.to_owned(),
        range: zero.clone(),
        selection_range: zero,
    }
}

fn global_item(name: &str, pack: &str) -> CallHierarchyItem {
    let zero = HierarchyRange { start_line: 0, start_col: 0, end_line: 0, end_col: 0 };
    CallHierarchyItem {
        name: name.to_owned(),
        kind: ItemKind::Function,
        detail: pack.to_owned(),
        uri: format!("jinja-builtin:{name}"),
        range: zero.clone(),
        selection_range: zero,
    }
}

/// Find the innermost macro in `idx` whose body span contains `span`.
fn find_enclosing_macro<'a>(idx: &'a TemplateIndex, span: &Span) -> Option<&'a MacroDefinition> {
    idx.macros
        .iter()
        .filter(|m| span_contains(&m.body, span))
        .min_by_key(|m| m.body.end_byte.saturating_sub(m.body.start_byte))
}

/// True when `outer` fully contains `inner` (by byte offsets).
fn span_contains(outer: &Span, inner: &Span) -> bool {
    outer.start_byte < outer.end_byte
        && outer.start_byte <= inner.start_byte
        && inner.end_byte <= outer.end_byte
}

fn byte_in_span(byte: usize, span: &Span) -> bool {
    span.start_byte < span.end_byte && span.start_byte <= byte && byte < span.end_byte
}

fn span_to_range(s: &Span) -> HierarchyRange {
    HierarchyRange {
        start_line: s.start_line,
        start_col: s.start_col,
        end_line: s.end_line,
        end_col: s.end_col,
    }
}

/// Search all workspace templates for a macro named `name`, returning the first match.
fn find_macro_in_workspace(
    workspace: &WorkspaceIndex,
    name: &str,
) -> Option<(String, MacroDefinition)> {
    workspace.templates.iter().find_map(|(path, idx)| {
        idx.macros
            .iter()
            .find(|m| m.name == name)
            .map(|m| (path.clone(), m.clone()))
    })
}

/// Resolve a reference name to the macro's definition item.
/// Checks: local macros → from-imported macros.
fn resolve_macro_item(
    name: &str,
    current_path: &str,
    index: &TemplateIndex,
    workspace: &WorkspaceIndex,
) -> Option<CallHierarchyItem> {
    // Local macro.
    if let Some(m) = index.macros.iter().find(|m| m.name == name) {
        return Some(macro_item(m, current_path));
    }

    // From-imported: {% from "source" import name %}.
    for fi in &index.from_imports {
        let is_imported = fi
            .names
            .iter()
            .any(|n| n.name == name || n.alias.as_deref() == Some(name));
        if !is_imported {
            continue;
        }
        if let Some(src_idx) = workspace.templates.get(&fi.source) {
            if let Some(m) = src_idx.macros.iter().find(|m| m.name == name) {
                return Some(macro_item(m, &fi.source));
            }
        }
    }

    // Workspace fallback: search all templates for a macro named `name`.
    if let Some((path, m)) = find_macro_in_workspace(workspace, name) {
        return Some(macro_item(&m, &path));
    }

    None
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
