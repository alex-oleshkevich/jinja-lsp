use std::collections::{HashMap, HashSet};

use super::symbols::{
    BlockDefinition, EnclosingOwner, FromImport, ImportAlias, MacroDefinition, Reference,
    ReferenceKind, Span, SyntaxError, TemplateRefKind, TemplateReference, VariableDefinition,
};
use crate::parsing::extract;

/// REQ-DATA-11: the definition a `Reference` binds to.
#[derive(Debug)]
pub enum ResolvedBinding<'a> {
    /// A scope-local or template-level variable definition.
    Variable(&'a VariableDefinition),
    /// A user-defined macro (local to this template or found in the workspace).
    Macro(&'a MacroDefinition),
    /// The name has no template-owned definition — it is host-injected context,
    /// an unresolved filter/test, or an un-hinted identifier.
    HostOwned,
}

/// REQ-DATA-08: everything known about one template, from its own parse tree.
#[derive(Debug, Clone)]
pub struct TemplateIndex {
    pub path: String,
    pub macros: Vec<MacroDefinition>,
    pub blocks: Vec<BlockDefinition>,
    pub variables: Vec<VariableDefinition>,
    pub import_aliases: Vec<ImportAlias>,
    pub from_imports: Vec<FromImport>,
    pub template_refs: Vec<TemplateReference>,
    pub references: Vec<Reference>,
    pub syntax_errors: Vec<SyntaxError>,
}

impl TemplateIndex {
    pub fn empty() -> Self {
        Self {
            path: String::new(),
            macros: vec![],
            blocks: vec![],
            variables: vec![],
            import_aliases: vec![],
            from_imports: vec![],
            template_refs: vec![],
            references: vec![],
            syntax_errors: vec![],
        }
    }

    /// Returns the single `Extends` reference if this template extends another.
    pub fn extends(&self) -> Option<&TemplateReference> {
        self.template_refs
            .iter()
            .find(|r| r.kind == TemplateRefKind::Extends && !r.is_dynamic)
    }

    /// REQ-DATA-11: resolve a reference to the definition it binds to.
    ///
    /// - Identifier → innermost `VariableDefinition` whose `valid_range` contains the ref's span.
    /// - Function → local `MacroDefinition`, then workspace-wide search.
    /// - Anything else (Attribute, Filter, Test) → `HostOwned` (resolved via registry/hints).
    pub fn resolve_reference<'a>(
        &'a self,
        reference: &Reference,
        workspace: &'a WorkspaceIndex,
    ) -> ResolvedBinding<'a> {
        match reference.kind {
            ReferenceKind::Identifier => {
                // Innermost binding: smallest valid_range that still contains the reference.
                self.variables
                    .iter()
                    .filter(|v| {
                        v.name == reference.name && range_contains(&v.valid_range, &reference.span)
                    })
                    .min_by_key(|v| {
                        v.valid_range.end_byte.saturating_sub(v.valid_range.start_byte)
                    })
                    .map(ResolvedBinding::Variable)
                    .unwrap_or(ResolvedBinding::HostOwned)
            }
            ReferenceKind::Function => {
                // Local macro first.
                if let Some(m) = self.macros.iter().find(|m| m.name == reference.name) {
                    return ResolvedBinding::Macro(m);
                }
                // From-imports: {% from "src" import name %} or {% from "src" import name as alias %}
                for fi in &self.from_imports {
                    let imported = fi.names.iter().any(|n| {
                        n.alias.as_deref().unwrap_or(n.name.as_str()) == reference.name
                            || n.name == reference.name
                    });
                    if imported {
                        if let Some(src_idx) = workspace.templates.get(&fi.source) {
                            // Find by original name (alias is local, original is in src).
                            let orig = fi.names.iter()
                                .find(|n| n.alias.as_deref().unwrap_or(n.name.as_str()) == reference.name)
                                .map(|n| n.name.as_str())
                                .unwrap_or(reference.name.as_str());
                            if let Some(m) = src_idx.macros.iter().find(|m| m.name == orig) {
                                return ResolvedBinding::Macro(m);
                            }
                        }
                    }
                }
                // Workspace-wide fallback.
                for idx in workspace.templates.values() {
                    if let Some(m) = idx.macros.iter().find(|m| m.name == reference.name) {
                        return ResolvedBinding::Macro(m);
                    }
                }
                ResolvedBinding::HostOwned
            }
            // Attributes, filters, tests are resolved via registry/hints — out of scope here.
            ReferenceKind::Attribute | ReferenceKind::Filter | ReferenceKind::Test => {
                ResolvedBinding::HostOwned
            }
        }
    }

    /// REQ-DATA-12: compute the innermost macro or block body containing `span`.
    /// Returns `Template` when no body encloses it.
    /// "Innermost" = smallest body (by byte length) that still contains `span`.
    pub fn enclosing_owner<'a>(&'a self, span: &Span) -> EnclosingOwner<'a> {
        // Collect all candidates (macros and blocks whose body contains span).
        let best_macro = self.macros.iter()
            .filter(|m| m.body.start_byte < m.body.end_byte && body_contains(&m.body, span))
            .min_by_key(|m| m.body.end_byte.saturating_sub(m.body.start_byte));

        let best_block = self.blocks.iter()
            .filter(|b| b.body.start_byte < b.body.end_byte && body_contains(&b.body, span))
            .min_by_key(|b| b.body.end_byte.saturating_sub(b.body.start_byte));

        match (best_macro, best_block) {
            (None, None) => EnclosingOwner::Template,
            (Some(m), None) => EnclosingOwner::Macro(m),
            (None, Some(b)) => EnclosingOwner::Block(b),
            (Some(m), Some(b)) => {
                // Both contain span; pick the smaller body (innermost).
                let m_len = m.body.end_byte.saturating_sub(m.body.start_byte);
                let b_len = b.body.end_byte.saturating_sub(b.body.start_byte);
                if m_len <= b_len { EnclosingOwner::Macro(m) } else { EnclosingOwner::Block(b) }
            }
        }
    }
}

fn body_contains(body: &Span, span: &Span) -> bool {
    body.start_byte <= span.start_byte && span.end_byte <= body.end_byte
}

fn range_contains(range: &Span, span: &Span) -> bool {
    range.start_byte <= span.start_byte && span.end_byte <= range.end_byte
}

/// REQ-DATA-09: maps each template path to its per-file index; resolved in Pass 2.
#[derive(Debug, Default, Clone)]
pub struct WorkspaceIndex {
    pub templates: HashMap<String, TemplateIndex>,
    /// REQ-EXTR-06: import graph — maps each template to the set of templates it
    /// statically references (extends/include/import/from, non-dynamic only).
    /// Populated by `relink()`; empty until first Pass 2 runs.
    pub import_graph: HashMap<String, Vec<String>>,
}

impl WorkspaceIndex {
    /// REQ-EXTR-05: index an inline Jinja region under `key` — identical to a file-based entry.
    pub fn index_inline(&mut self, key: &str, source: &str) {
        let mut idx = extract(source);
        idx.path = key.to_owned();
        self.templates.insert(key.to_owned(), idx);
    }

    /// REQ-EXTR-06: rebuild the import graph from all `TemplateIndex` entries.
    /// Only static (non-dynamic) references are included.
    pub fn relink(&mut self) {
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        for (path, idx) in &self.templates {
            let targets: Vec<String> = idx
                .template_refs
                .iter()
                .filter(|r| !r.is_dynamic)
                .map(|r| r.path.clone())
                .collect();
            graph.insert(path.clone(), targets);
        }
        self.import_graph = graph;
    }

    /// REQ-EXTR-06: return `true` if `start` can reach itself through the import graph.
    pub fn has_import_cycle(&self, start: &str) -> bool {
        let mut in_path: HashSet<String> = HashSet::new();
        let mut done: HashSet<String> = HashSet::new();
        self.dfs_has_cycle(start, &mut in_path, &mut done)
    }

    // DFS with two sets: `in_path` = currently on the recursion stack (true cycle if revisited),
    // `done` = fully explored (safe to skip). A single-set approach false-positives on diamonds.
    fn dfs_has_cycle(&self, node: &str, in_path: &mut HashSet<String>, done: &mut HashSet<String>) -> bool {
        if in_path.contains(node) {
            return true;
        }
        if done.contains(node) {
            return false;
        }
        in_path.insert(node.to_owned());
        let refs: Vec<String> = self.import_graph.get(node).cloned().unwrap_or_default();
        for target in &refs {
            if self.dfs_has_cycle(target, in_path, done) {
                return true;
            }
        }
        in_path.remove(node);
        done.insert(node.to_owned());
        false
    }

    /// REQ-DATA-10: ordered extends lineage from `path` up to the root template.
    pub fn template_chain(&self, path: &str) -> Vec<String> {
        let mut chain = Vec::new();
        let mut current = path.to_owned();
        let mut seen = HashSet::new();

        loop {
            let key = match self.resolve_key(&current) {
                Some(k) => k.to_owned(),
                None => break,
            };
            if !seen.insert(key.clone()) {
                break; // cycle guard
            }
            chain.push(key.clone());
            match self.templates.get(&key).and_then(|idx| idx.extends()) {
                Some(r) => current = r.path.clone(),
                None => break,
            }
        }
        chain
    }

    // Maps an extends target (relative path) to the actual map key, handling
    // the mismatch between relative keys (build_workspace) and absolute keys (server).
    fn resolve_key<'a>(&'a self, target: &'a str) -> Option<&'a str> {
        if self.templates.contains_key(target) {
            return Some(target);
        }
        let suffix_fwd = format!("/{target}");
        let suffix_bwd = format!("\\{target}");
        self.templates
            .keys()
            .find(|k| k.ends_with(&suffix_fwd) || k.ends_with(&suffix_bwd))
            .map(|k| k.as_str())
    }
}
