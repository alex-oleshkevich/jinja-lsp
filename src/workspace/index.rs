use std::collections::{HashMap, HashSet};

use super::symbols::{
    BlockDefinition, FromImport, ImportAlias, MacroDefinition, Reference, SyntaxError,
    TemplateRefKind, TemplateReference, VariableDefinition,
};

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
    /// Returns the single `Extends` reference if this template extends another.
    pub fn extends(&self) -> Option<&TemplateReference> {
        self.template_refs
            .iter()
            .find(|r| r.kind == TemplateRefKind::Extends && !r.is_dynamic)
    }
}

/// REQ-DATA-09: maps each template path to its per-file index; resolved in Pass 2.
#[derive(Debug, Default)]
pub struct WorkspaceIndex {
    pub templates: HashMap<String, TemplateIndex>,
}

impl WorkspaceIndex {
    /// REQ-DATA-10: ordered extends lineage from `path` up to the root template.
    pub fn template_chain(&self, path: &str) -> Vec<String> {
        let mut chain = Vec::new();
        let mut current = path.to_owned();
        let mut seen = HashSet::new();

        loop {
            if !seen.insert(current.clone()) {
                break; // cycle guard
            }
            chain.push(current.clone());
            match self.templates.get(&current).and_then(|idx| idx.extends()) {
                Some(r) => current = r.path.clone(),
                None => break,
            }
        }
        chain
    }
}
