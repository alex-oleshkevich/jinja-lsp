// REQ-ACT-11: Rename symbol — workspace-wide for definitions, scope-local for locals.

use std::collections::HashMap;

use crate::edit::{TextEdit, WorkspaceEdit};
use crate::workspace::{
    index::{TemplateIndex, WorkspaceIndex},
    symbols::{ReferenceKind, Span},
};

pub fn layer_name() -> &'static str {
    "rename"
}

/// The scope of a rename operation.
#[derive(Debug, Clone, PartialEq)]
pub enum RenameTarget {
    /// A local variable, parameter, or binding — rename within this template only.
    /// `scope`: `Some(valid_range)` constrains edits to references within that range;
    /// `None` renames all occurrences in the file (backward-compatible).
    Local { scope: Option<Span> },
    /// A macro or block definition — rename across the whole workspace.
    Workspace,
}

/// Check if the cursor at (line, col) sits on a renameable symbol.
///
/// Returns `Some((target, name))` when a symbol is found, `None` otherwise.
pub fn rename_at_cursor(
    source: &str,
    _file: &str,
    line: u32,
    col: u32,
    index: &TemplateIndex,
    _workspace: &WorkspaceIndex,
) -> Option<(RenameTarget, String)> {
    // Convert (line, col) to byte offset and find the identifier word there.
    let byte = line_col_to_byte(source, line, col)?;

    // Name-based matches must be guarded: HTML text matching a block/macro/var
    // name must not trigger a rename outside Jinja delimiters.
    let in_jinja = super::inside_jinja(source, byte);

    if in_jinja {
        let word = super::word_at_byte(source, byte);
        if word.is_empty() {
            return None;
        }

        // If the word matches a macro definition, it's a workspace-wide rename.
        if index.macros.iter().any(|m| m.name == word) {
            return Some((RenameTarget::Workspace, word.to_owned()));
        }

        // If the word matches a block definition, it's workspace-wide.
        if index.blocks.iter().any(|b| b.name == word) {
            return Some((RenameTarget::Workspace, word.to_owned()));
        }
    }

    // Reference span check is position-based (parser-assigned spans are always inside Jinja).
    let ref_ = index.references.iter().find(|r| {
        r.span.start_line == line
            && col >= r.span.start_col
            && col < r.span.start_col + r.name.len() as u32
    });

    if let Some(r) = ref_ {
        if r.kind == ReferenceKind::Identifier {
            let scope = tightest_scope_for(&r.name, byte, index);
            return Some((RenameTarget::Local { scope }, r.name.clone()));
        }
    }

    // Variable name match — also guard against HTML text.
    if in_jinja {
        let word = super::word_at_byte(source, byte);
        if index.variables.iter().any(|v| v.name == word) {
            let scope = tightest_scope_for(word, byte, index);
            return Some((RenameTarget::Local { scope }, word.to_owned()));
        }
    }

    None
}

/// Find the narrowest (smallest valid_range) VariableDefinition binding
/// whose name == `name` and whose valid_range contains `byte`.
fn tightest_scope_for(name: &str, byte: usize, index: &TemplateIndex) -> Option<Span> {
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

/// Validate `new_name` and check for scope collisions before producing edits.
///
/// Returns `Some(message)` if the rename should be refused — the caller must surface
/// this as a `window/showMessage` notification and produce no edit.
/// Returns `None` if the rename is valid and should proceed.
pub fn check_rename_preconditions(
    new_name: &str,
    target: &RenameTarget,
    index: &TemplateIndex,
) -> Option<String> {
    if !is_valid_jinja_identifier(new_name) {
        return Some(format!(
            "'{new_name}' is not a valid Jinja identifier (must match [a-zA-Z_][a-zA-Z0-9_]*)"
        ));
    }
    if let RenameTarget::Local { scope } = target {
        if has_collision(new_name, scope.as_ref(), index) {
            return Some(format!(
                "'{new_name}' already binds in the same scope — rename would create a collision"
            ));
        }
    }
    None
}

/// A valid Jinja identifier starts with a letter or underscore, followed by letters, digits, or underscores.
pub fn is_valid_jinja_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Returns true if any existing VariableDefinition named `new_name` has a `valid_range`
/// overlapping with `scope`. Two ranges overlap when neither ends before the other starts.
fn has_collision(new_name: &str, scope: Option<&Span>, index: &TemplateIndex) -> bool {
    let (scope_start, scope_end) = match scope {
        Some(s) => (s.start_byte, s.end_byte),
        None => (0, usize::MAX), // whole-file scope overlaps with everything
    };
    index.variables.iter().any(|v| {
        v.name == new_name
            && v.valid_range.start_byte < scope_end
            && v.valid_range.end_byte > scope_start
    })
}

/// Compute the WorkspaceEdit for renaming `old_name` → `new_name`.
///
/// For `Local` target: rewrites all identifier references in `file`.
/// For `Workspace` target: rewrites all references in the whole workspace.
/// `sources` must map every template path involved to its full text — definition
/// edits (macro/block names) need the source to locate the name past the keyword.
pub fn compute_rename(
    sources: &HashMap<String, String>,
    file: &str,
    old_name: &str,
    new_name: &str,
    target: RenameTarget,
    index: &TemplateIndex,
    workspace: &WorkspaceIndex,
) -> Option<WorkspaceEdit> {
    let mut changes: HashMap<String, Vec<TextEdit>> = HashMap::new();

    match target {
        RenameTarget::Local { scope } => {
            let edits = rename_in_index_scoped(old_name, new_name, index, scope.as_ref());
            if !edits.is_empty() {
                changes.insert(file.to_owned(), edits);
            }
        }
        RenameTarget::Workspace => {
            if let Some(source) = sources.get(file) {
                let def_edits = rename_in_index(source, old_name, new_name, index);
                if !def_edits.is_empty() {
                    changes.insert(file.to_owned(), def_edits);
                }
            }
            // Skip templates that define their own macro named old_name — those would be
            // independent macros that happen to share the name, not callers of this one.
            for (path, tpl) in &workspace.templates {
                if path == file {
                    continue;
                }
                if !super::template_does_not_shadow_macro(tpl, old_name) {
                    continue;
                }
                let Some(tpl_source) = sources.get(path) else { continue };
                let edits = rename_in_index(tpl_source, old_name, new_name, tpl);
                if !edits.is_empty() {
                    changes.insert(path.clone(), edits);
                }
            }
        }
    }

    if changes.is_empty() {
        None
    } else {
        Some(WorkspaceEdit { changes, create_files: vec![] })
    }
}

/// Convert (line, col) to a byte offset in `source`.
fn line_col_to_byte(source: &str, line: u32, col: u32) -> Option<usize> {
    let line_start: usize = source.split('\n')
        .take(line as usize)
        .map(|l| l.len() + 1) // +1 for the '\n'
        .sum();
    let byte = line_start + col as usize;
    if byte <= source.len() { Some(byte) } else { None }
}

/// Locate `old_name` inside the `{% macro … %}` / `{% block … %}` tag starting at
/// `tag_start_byte` (past the keyword) and build the rename TextEdit for it.
fn definition_name_edit(
    source: &str,
    tag_start_byte: usize,
    old_name: &str,
    new_name: &str,
) -> Option<TextEdit> {
    let name_byte = super::find_name_in_tag(source, tag_start_byte, old_name)?;
    let (line, col) = byte_to_line_col(source, name_byte);
    Some(TextEdit {
        start_line: line,
        start_col: col,
        end_line: line,
        end_col: col + old_name.len() as u32,
        new_text: new_name.to_owned(),
    })
}

fn byte_to_line_col(source: &str, byte: usize) -> (u32, u32) {
    let mut line = 0u32;
    let mut col = 0u32;
    let mut pos = 0usize;
    for ch in source.chars() {
        if pos >= byte {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += ch.len_utf8() as u32;
        }
        pos += ch.len_utf8();
    }
    (line, col)
}

/// Scoped version: only renames identifier/function references whose start_byte falls within `scope`.
/// Macro/block definitions and from-import names are always included (scope does not apply to them
/// because local-rename callers only match local variables, which appear as identifier references).
fn rename_in_index_scoped(old_name: &str, new_name: &str, index: &TemplateIndex, scope: Option<&Span>) -> Vec<TextEdit> {
    let in_scope = |ref_span: &Span| -> bool {
        match scope {
            None => true,
            Some(vr) => ref_span.start_byte >= vr.start_byte && ref_span.start_byte <= vr.end_byte,
        }
    };

    let mut edits: Vec<TextEdit> = Vec::new();

    // For identifier references, apply the scope filter.
    for r in &index.references {
        if r.name == old_name
            && matches!(r.kind, ReferenceKind::Identifier | ReferenceKind::Function)
            && in_scope(&r.span)
        {
            edits.push(TextEdit {
                start_line: r.span.start_line,
                start_col: r.span.start_col,
                end_line: r.span.start_line,
                end_col: r.span.start_col + old_name.len() as u32,
                new_text: new_name.to_owned(),
            });
        }
    }

    edits.sort_by_key(|e| (e.start_line, e.start_col));
    edits.dedup_by_key(|e| (e.start_line, e.start_col));
    edits
}

/// Collect TextEdits for all occurrences of `old_name` as an identifier or macro call in `index`.
///
/// `source` is `index`'s own full text — needed to locate macro/block definition names,
/// whose spans start at the enclosing keyword (`{% macro …`, `{% block …`), not the name.
fn rename_in_index(source: &str, old_name: &str, new_name: &str, index: &TemplateIndex) -> Vec<TextEdit> {
    let mut edits: Vec<TextEdit> = Vec::new();

    // Macro definition names.
    for m in &index.macros {
        if m.name == old_name {
            if let Some(edit) = definition_name_edit(source, m.span.start_byte, old_name, new_name) {
                edits.push(edit);
            }
        }
    }

    // Block definition names (opening tag + optional trailing endblock name).
    for b in &index.blocks {
        if b.name == old_name {
            if let Some(edit) = definition_name_edit(source, b.span.start_byte, old_name, new_name) {
                edits.push(edit);
            }
            if let Some(ref ens) = b.end_name_span {
                edits.push(TextEdit {
                    start_line: ens.start_line,
                    start_col: ens.start_col,
                    end_line: ens.start_line,
                    end_col: ens.start_col + old_name.len() as u32,
                    new_text: new_name.to_owned(),
                });
            }
        }
    }

    // All identifier and macro-call (Function-kind) references.
    for r in &index.references {
        if r.name == old_name && matches!(r.kind, ReferenceKind::Identifier | ReferenceKind::Function) {
            edits.push(TextEdit {
                start_line: r.span.start_line,
                start_col: r.span.start_col,
                end_line: r.span.start_line,
                end_col: r.span.start_col + old_name.len() as u32,
                new_text: new_name.to_owned(),
            });
        }
    }

    // From-import name positions (e.g. `from "x.html" import old_name as alias`).
    for fi in &index.from_imports {
        for iname in &fi.names {
            if iname.name == old_name {
                edits.push(TextEdit {
                    start_line: iname.name_span.start_line,
                    start_col: iname.name_span.start_col,
                    end_line: iname.name_span.start_line,
                    end_col: iname.name_span.start_col + old_name.len() as u32,
                    new_text: new_name.to_owned(),
                });
            }
        }
    }

    edits.sort_by_key(|e| (e.start_line, e.start_col));
    edits.dedup_by_key(|e| (e.start_line, e.start_col));
    edits
}
