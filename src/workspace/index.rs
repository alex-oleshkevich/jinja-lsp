use std::collections::{HashMap, HashSet};

use super::inline::InlineRange;
use super::symbols::{
    BlockDefinition, EnclosingOwner, FromImport, ImportAlias, MacroCallSite, MacroDefinition,
    Reference, ReferenceKind, Span, SyntaxError, TemplateRefKind, TemplateReference,
    VariableDefinition,
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
    /// Path relative to the discovering templates-root directory (e.g. `"blog/post.html"`),
    /// as written in `{% extends %}`/`{% include %}`. `None` when the workspace is keyed
    /// by relative paths already (`path` doubles as the relative form in that case) or the
    /// template was indexed outside `build_workspace`/`build_workspace_abs` (e.g. inline).
    pub relative_path: Option<String>,
    pub macros: Vec<MacroDefinition>,
    pub blocks: Vec<BlockDefinition>,
    pub variables: Vec<VariableDefinition>,
    pub import_aliases: Vec<ImportAlias>,
    pub from_imports: Vec<FromImport>,
    pub template_refs: Vec<TemplateReference>,
    pub references: Vec<Reference>,
    pub macro_calls: Vec<MacroCallSite>,
    pub syntax_errors: Vec<SyntaxError>,
}

impl TemplateIndex {
    pub fn empty() -> Self {
        Self {
            path: String::new(),
            relative_path: None,
            macros: vec![],
            blocks: vec![],
            variables: vec![],
            import_aliases: vec![],
            from_imports: vec![],
            template_refs: vec![],
            references: vec![],
            macro_calls: vec![],
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
                        if let Some(src_idx) = workspace.get_by_ref(&fi.source) {
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
                if let Some(m) = workspace.find_macro_workspace_wide(&reference.name) {
                    return ResolvedBinding::Macro(m);
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

/// REQ-DATA-09: maps each template path to its per-file index.
#[derive(Debug, Default, Clone)]
pub struct WorkspaceIndex {
    pub templates: HashMap<String, TemplateIndex>,
    /// REQ-INLN-03: host-coordinate metadata for each inline template entry.
    /// Keyed by the inline template key (e.g. `/path/view.py::47`).
    pub inline_ranges: HashMap<String, InlineRange>,
}

impl WorkspaceIndex {
    /// REQ-EXTR-05: index an inline Jinja region under `key` — identical to a file-based entry.
    pub fn index_inline(&mut self, key: &str, source: &str) {
        let mut idx = extract(source);
        idx.path = key.to_owned();
        self.templates.insert(key.to_owned(), idx);
    }

    /// REQ-INLN-03: register host-coordinate metadata for an inline region key.
    pub fn register_inline_range(&mut self, key: &str, range: InlineRange) {
        self.inline_ranges.insert(key.to_owned(), range);
    }

    /// REQ-INLN-03: iterate over all inline regions belonging to `host_key`.
    pub fn inline_ranges_for<'a>(&'a self, host_key: &'a str)
        -> impl Iterator<Item = (&'a str, &'a InlineRange)> + 'a
    {
        self.inline_ranges.iter()
            .filter(move |(_, r)| r.host_path == host_key)
            .map(|(k, r)| (k.as_str(), r))
    }

    /// REQ-INLN-03: find the inline region that contains `host_byte` in `host_key`,
    /// and return `(inline_key, inline_line, inline_col)`.
    pub fn resolve_inline_cursor<'a>(
        &'a self,
        host_key: &'a str,
        host_line: u32,
        host_col: u32,
        host_byte: usize,
    ) -> Option<(&'a str, u32, u32)> {
        for (ikey, range) in self.inline_ranges_for(host_key) {
            if range.contains_host_byte(host_byte) {
                if let Some((il, ic)) = range.to_inline_position(host_line, host_col) {
                    return Some((ikey, il, ic));
                }
            }
        }
        None
    }

    /// REQ-DATA-10: ordered extends lineage from `path` up to the root template.
    pub fn template_chain(&self, path: &str) -> Vec<String> {
        let mut chain = Vec::new();
        let mut current = path.to_owned();
        let mut seen = HashSet::new();

        while let Some(k) = self.resolve_key(&current) {
            let key = k.to_owned();
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

    /// Look up a template by a reference path that may be relative (`"base.html"`)
    /// even when the workspace is keyed by absolute paths (LSP server path).
    ///
    /// Returns `None` when no template in the workspace matches `ref_path`.
    pub fn get_by_ref(&self, ref_path: &str) -> Option<&TemplateIndex> {
        self.resolve_key(ref_path).and_then(|k| self.templates.get(k))
    }

    /// Workspace-wide macro-name fallback search, used when a callee is neither a
    /// local macro nor an explicit import. `templates` is a `HashMap`, so iterating
    /// it directly picks an arbitrary match when the same macro name is defined in
    /// more than one template; sort by template key first so the result is stable
    /// across runs regardless of hash iteration order.
    pub(crate) fn find_macro_workspace_wide(&self, name: &str) -> Option<&MacroDefinition> {
        let mut keys: Vec<&str> = self.templates.keys().map(String::as_str).collect();
        keys.sort_unstable();
        keys.into_iter()
            .find_map(|k| self.templates[k].macros.iter().find(|m| m.name == name))
    }

    // Maps an extends target (relative path) to the actual map key, handling
    // the mismatch between relative keys (build_workspace) and absolute keys (server).
    //
    // When `target` isn't an exact key, multiple templates can share a basename
    // (e.g. `app1/base.html` and `app2/base.html` both satisfy `"base.html"`).
    // Picking the first HashMap-iteration hit made resolution flaky across runs;
    // pick the shortest matching key instead (the closest match), tie-broken
    // lexicographically so the result is always the same regardless of hash
    // iteration order. No format! allocation: a suffix match requires the byte
    // just before it to be a path separator.
    pub(crate) fn resolve_key<'a>(&'a self, target: &'a str) -> Option<&'a str> {
        if self.templates.contains_key(target) {
            return Some(target);
        }
        self.templates
            .keys()
            .filter(|k| {
                k.len() > target.len()
                    && k.ends_with(target)
                    && matches!(k.as_bytes()[k.len() - target.len() - 1], b'/' | b'\\')
            })
            .min_by_key(|k| (k.len(), k.as_str()))
            .map(|k| k.as_str())
    }
}
