// F15 — Code lens: reference-count and inheritance lenses. REQ-LENS-01..05.

use crate::workspace::{
    index::{TemplateIndex, WorkspaceIndex},
    symbols::ReferenceKind,
};

// ── Public types ──────────────────────────────────────────────────────────────

/// Which kind of thing the lens is anchored to.
#[derive(Debug, Clone, PartialEq)]
pub enum LensSymbolKind {
    Macro,
    Block,
}

/// Three distinct lens kinds that can be emitted.
#[derive(Debug, Clone, PartialEq)]
pub enum LensKind {
    ReferenceCount,
    InheritanceOverrides,
    InheritanceExtended,
}

/// Stable symbol identity carried in the opaque `data` field (REQ-LENS-04).
/// Resolve matches by (symbol_kind, symbol_name) — never by exact byte position.
#[derive(Debug, Clone)]
pub struct LensData {
    pub file_path: String,
    pub symbol_kind: LensSymbolKind,
    pub symbol_name: String,
    pub decl_line: u32,
    pub decl_col: u32,
    pub lens_kind: LensKind,
}

/// A single code lens. `title` is `None` in the initial listing (Anchored state)
/// and `Some(text)` after resolve. An empty string title means the lens is suppressed.
#[derive(Debug, Clone)]
pub struct CodeLens {
    pub line: u32,
    pub col: u32,
    pub title: Option<String>,
    pub data: LensData,
}

/// Independent on/off switches per lens kind (REQ-LENS-03).
#[derive(Debug, Clone)]
pub struct CodeLensConfig {
    pub references: bool,
    pub inheritance: bool,
}

impl Default for CodeLensConfig {
    fn default() -> Self {
        Self {
            references: true,
            inheritance: true,
        }
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Emit the initial cheap listing — one stub per eligible symbol, no title (REQ-LENS-04).
/// Kinds disabled by `config` are omitted entirely (REQ-LENS-03).
pub fn code_lens(
    template_path: &str,
    index: &TemplateIndex,
    config: &CodeLensConfig,
) -> Vec<CodeLens> {
    if !config.references && !config.inheritance {
        return vec![];
    }
    let mut out = Vec::new();

    for m in &index.macros {
        if config.references {
            out.push(make_lens(
                template_path,
                LensSymbolKind::Macro,
                &m.name,
                m.span.start_line,
                m.span.start_col,
                LensKind::ReferenceCount,
            ));
        }
    }

    for b in &index.blocks {
        if config.references {
            out.push(make_lens(
                template_path,
                LensSymbolKind::Block,
                &b.name,
                b.span.start_line,
                b.span.start_col,
                LensKind::ReferenceCount,
            ));
        }
        if config.inheritance {
            out.push(make_lens(
                template_path,
                LensSymbolKind::Block,
                &b.name,
                b.span.start_line,
                b.span.start_col,
                LensKind::InheritanceOverrides,
            ));
            out.push(make_lens(
                template_path,
                LensSymbolKind::Block,
                &b.name,
                b.span.start_line,
                b.span.start_col,
                LensKind::InheritanceExtended,
            ));
        }
    }

    out
}

/// Resolve a lens: compute its title from the workspace graph (REQ-LENS-04).
/// Returns the lens with `title` set. Empty string = suppressed (REQ-LENS-05, §10).
pub fn code_lens_resolve(mut lens: CodeLens, workspace: &WorkspaceIndex) -> CodeLens {
    let file_path = lens.data.file_path.clone();
    let name = lens.data.symbol_name.clone();

    lens.title = Some(match lens.data.lens_kind {
        LensKind::ReferenceCount => {
            // Verify the symbol still exists in the workspace (P3).
            let exists = symbol_exists(workspace, &file_path, &lens.data.symbol_kind, &name);
            if !exists {
                String::new() // stale symbol
            } else {
                let count = count_references(workspace, &lens.data.symbol_kind, &file_path, &name);
                if count == 0 {
                    String::new() // suppressed (REQ-LENS-05)
                } else if count == 1 {
                    "1 reference".to_owned()
                } else {
                    format!("{count} references")
                }
            }
        }
        LensKind::InheritanceOverrides => {
            if has_ancestor_block(workspace, &file_path, &name) {
                "overrides base".to_owned()
            } else {
                String::new()
            }
        }
        LensKind::InheritanceExtended => {
            let count = count_descendant_overrides(workspace, &file_path, &name);
            if count == 0 {
                String::new()
            } else {
                format!("extended by {count}")
            }
        }
    });

    lens
}

// ── Internal helpers ──────────────────────────────────────────────────────────

fn make_lens(
    file_path: &str,
    symbol_kind: LensSymbolKind,
    name: &str,
    line: u32,
    col: u32,
    lens_kind: LensKind,
) -> CodeLens {
    CodeLens {
        line,
        col,
        title: None,
        data: LensData {
            file_path: file_path.to_owned(),
            symbol_kind,
            symbol_name: name.to_owned(),
            decl_line: line,
            decl_col: col,
            lens_kind,
        },
    }
}

/// True when the workspace still contains the symbol by (kind, name) — ignores position.
fn symbol_exists(
    workspace: &WorkspaceIndex,
    file_path: &str,
    kind: &LensSymbolKind,
    name: &str,
) -> bool {
    let Some(idx) = workspace.templates.get(file_path) else {
        return false;
    };
    match kind {
        LensSymbolKind::Macro => idx.macros.iter().any(|m| m.name == name),
        LensSymbolKind::Block => idx.blocks.iter().any(|b| b.name == name),
    }
}

/// Count how many places in the workspace reference the symbol (REQ-LENS-01).
/// For macros: Identifier or Function references by name across all templates.
/// For blocks: number of descendant templates that override the same block.
fn count_references(
    workspace: &WorkspaceIndex,
    kind: &LensSymbolKind,
    file_path: &str,
    name: &str,
) -> usize {
    match kind {
        LensSymbolKind::Macro => workspace
            .templates
            .iter()
            .filter(|(path, tmpl_idx)| {
                path.as_str() == file_path || super::template_does_not_shadow_macro(tmpl_idx, name)
            })
            .flat_map(|(_, idx)| idx.references.iter())
            .filter(|r| {
                r.name == name
                    && matches!(r.kind, ReferenceKind::Identifier | ReferenceKind::Function)
            })
            .count(),
        LensSymbolKind::Block => count_descendant_overrides(workspace, file_path, name),
    }
}

/// True if any ancestor template (above `file_path` in the extends chain) defines `block_name`.
fn has_ancestor_block(workspace: &WorkspaceIndex, file_path: &str, block_name: &str) -> bool {
    let chain = workspace.template_chain(file_path);
    // chain[0] == file_path itself; skip it and check ancestors.
    for ancestor in chain.iter().skip(1) {
        if let Some(idx) = workspace.templates.get(ancestor) {
            if idx.blocks.iter().any(|b| b.name == block_name) {
                return true;
            }
        }
    }
    false
}

/// Count all templates that (a) define `block_name` AND (b) are descendants of `file_path`.
/// A template T is a descendant of `file_path` iff `file_path` appears in T's extends chain.
/// This counts ALL descendants, not only immediate children (deep-chain rule, REQ-LENS-02).
fn count_descendant_overrides(
    workspace: &WorkspaceIndex,
    file_path: &str,
    block_name: &str,
) -> usize {
    workspace
        .templates
        .iter()
        .filter(|(path, _)| path.as_str() != file_path)
        .filter(|(_, idx)| idx.blocks.iter().any(|b| b.name == block_name))
        .filter(|(path, _)| {
            workspace
                .template_chain(path)
                .iter()
                .any(|p| p == file_path)
        })
        .count()
}
