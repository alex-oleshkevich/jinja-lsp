// F16 — Call hierarchy: prepare/incoming/outgoing for macros. REQ-CALL-01..04.

use std::collections::HashMap;

use crate::{
    builtins::registry::{Category, Registry, Source},
    workspace::{
        index::{TemplateIndex, WorkspaceIndex},
        symbols::{EnclosingOwner, MacroDefinition, ReferenceKind, Span, TemplateRefKind},
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
    let byte = super::line_col_to_byte(source, line, col);

    // 1. Check references first (more specific — identifies which symbol at cursor).
    let ref_at = index
        .references
        .iter()
        .filter(|r| matches!(r.kind, ReferenceKind::Function | ReferenceKind::Identifier))
        .find(|r| super::byte_in_span(byte, &r.span));

    if let Some(r) = ref_at {
        if let Some(item) = resolve_macro_item(&r.name, path, index, workspace) {
            return vec![item];
        }
        // Found a reference but it's not a user macro (e.g. a registry global).
        return vec![];
    }

    // 2. Check macro definitions (cursor on the definition header itself).
    if let Some(m) = index.macros.iter().find(|m| super::byte_in_span(byte, &m.span)) {
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
        // Skip templates that define their own local macro with this name — those
        // calls resolve to the local definition, not the queried macro (matches
        // code_lens's count_references so the lens count and hierarchy agree).
        if tpl_path.as_str() != item.uri && !super::template_does_not_shadow_macro(tmpl_idx, macro_name) {
            continue;
        }
        for r in &tmpl_idx.references {
            if r.name != *macro_name {
                continue;
            }
            if !matches!(r.kind, ReferenceKind::Function | ReferenceKind::Identifier) {
                continue;
            }

            let enclosing = tmpl_idx.enclosing_owner(&r.span);
            let enclosing_name = if let EnclosingOwner::Macro(m) = &enclosing { Some(m.name.clone()) } else { None };
            let key = (tpl_path.clone(), enclosing_name);

            let from_range = span_to_range(&r.span);
            let entry = groups.entry(key).or_insert_with(|| {
                let owner = match &enclosing {
                    EnclosingOwner::Macro(m) => macro_item(m, tpl_path),
                    _ => template_item(tpl_path),
                };
                (owner, vec![])
            });
            entry.1.push(from_range);
        }
    }

    let mut calls: Vec<IncomingCall> = groups
        .into_values()
        .map(|(from, from_ranges)| IncomingCall { from, from_ranges })
        .collect();
    // jinja-lsp-x6e9: HashMap::into_values has no stable order — sort for
    // byte-for-byte determinism so the call list doesn't jump around in the
    // editor UI between identical requests (matches symbols.rs's precedent).
    calls.sort_by(|a, b| (&a.from.uri, &a.from.name).cmp(&(&b.from.uri, &b.from.name)));
    calls
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
        if !body.contains(&r.span) {
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
        } else if let Some(entry) = registry.get(Category::Function, &r.name) {
            // Registry global — terminal, synthetic URI.
            let pack = if let Source::Pack(p) = &entry.source { p.as_str() } else { "global" };
            global_item(&r.name, pack)
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
        if !body.contains(&tref.span) {
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

    let mut calls: Vec<OutgoingCall> = edges
        .into_values()
        .map(|(to, from_ranges)| OutgoingCall { to, from_ranges })
        .collect();
    // jinja-lsp-x6e9: same determinism fix as incoming_calls.
    calls.sort_by(|a, b| (&a.to.uri, &a.to.name).cmp(&(&b.to.uri, &b.to.name)));
    calls
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn macro_item(m: &MacroDefinition, path: &str) -> CallHierarchyItem {
    CallHierarchyItem {
        name: m.name.clone(),
        kind: ItemKind::Function,
        detail: path.to_owned(),
        uri: path.to_owned(),
        range: span_to_range(&m.span),
        selection_range: span_to_range(&m.name_span),
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
        detail: format!("global - {pack} pack"),
        uri: format!("jinja-builtin:{pack}/{name}"),
        range: zero.clone(),
        selection_range: zero,
    }
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

    // From-imported: {% from "source" import name %} or {% from "source" import name as alias %}.
    for fi in &index.from_imports {
        // Resolve alias to the real imported name for lookup in the source template.
        let real_name = fi.names.iter()
            .find(|n| n.name == name || n.alias.as_deref() == Some(name))
            .map(|n| n.name.as_str());
        let Some(real_name) = real_name else { continue };
        if let Some(src_key) = workspace.resolve_key(&fi.source) {
            if let Some(src_idx) = workspace.templates.get(src_key) {
                if let Some(m) = src_idx.macros.iter().find(|m| m.name == real_name) {
                    return Some(macro_item(m, src_key));
                }
            }
        }
    }

    // Workspace fallback: search all templates for a macro named `name`.
    if let Some((path, m)) = find_macro_in_workspace(workspace, name) {
        return Some(macro_item(&m, &path));
    }

    None
}
