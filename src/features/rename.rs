// REQ-ACT-11: Rename symbol — workspace-wide for definitions, scope-local for locals.

use std::collections::HashMap;

use crate::features::code_actions::{TextEdit, WorkspaceEdit};
use crate::workspace::{
    index::{TemplateIndex, WorkspaceIndex},
    symbols::ReferenceKind,
};

pub fn layer_name() -> &'static str {
    "rename"
}

/// The scope of a rename operation.
#[derive(Debug, Clone, PartialEq)]
pub enum RenameTarget {
    /// A local variable, parameter, or binding — rename within this template only.
    Local,
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
    let word = word_at_byte(source, byte);
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

    // Otherwise check if there's a tracked reference at this position.
    let ref_ = index.references.iter().find(|r| {
        r.span.start_line == line
            && col >= r.span.start_col
            && col < r.span.start_col + r.name.len() as u32
    });

    if let Some(r) = ref_ {
        if r.kind == ReferenceKind::Identifier {
            return Some((RenameTarget::Local, r.name.clone()));
        }
    }

    // Also allow cursor on a word that matches any variable definition by name.
    if index.variables.iter().any(|v| v.name == word) {
        return Some((RenameTarget::Local, word.to_owned()));
    }

    None
}

/// Compute the WorkspaceEdit for renaming `old_name` → `new_name`.
///
/// For `Local` target: rewrites all identifier references in `file`.
/// For `Workspace` target: rewrites all references in the whole workspace.
pub fn compute_rename(
    _source: &str,
    file: &str,
    old_name: &str,
    new_name: &str,
    target: RenameTarget,
    index: &TemplateIndex,
    workspace: &WorkspaceIndex,
) -> Option<WorkspaceEdit> {
    let mut changes: HashMap<String, Vec<TextEdit>> = HashMap::new();

    match target {
        RenameTarget::Local => {
            let edits = rename_in_index(old_name, new_name, index);
            if !edits.is_empty() {
                changes.insert(file.to_owned(), edits);
            }
        }
        RenameTarget::Workspace => {
            let def_edits = rename_in_index(old_name, new_name, index);
            if !def_edits.is_empty() {
                changes.insert(file.to_owned(), def_edits);
            }
            for (path, tpl) in &workspace.templates {
                if path == file {
                    continue;
                }
                let edits = rename_in_index(old_name, new_name, tpl);
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

/// Extract the Jinja identifier word centered at `byte` in `source`.
fn word_at_byte(source: &str, byte: usize) -> &str {
    let start = source[..byte]
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    let end = source[byte..]
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| byte + i)
        .unwrap_or(source.len());
    &source[start..end]
}

/// Collect TextEdits for all occurrences of `old_name` as an identifier in `index`.
fn rename_in_index(old_name: &str, new_name: &str, index: &TemplateIndex) -> Vec<TextEdit> {
    let mut edits: Vec<TextEdit> = Vec::new();

    // Macro definition names.
    for m in &index.macros {
        if m.name == old_name {
            edits.push(TextEdit {
                start_line: m.span.start_line,
                start_col: m.span.start_col,
                end_line: m.span.start_line,
                end_col: m.span.start_col + old_name.len() as u32,
                new_text: new_name.to_owned(),
            });
        }
    }

    // Block definition names.
    for b in &index.blocks {
        if b.name == old_name {
            edits.push(TextEdit {
                start_line: b.span.start_line,
                start_col: b.span.start_col,
                end_line: b.span.start_line,
                end_col: b.span.start_col + old_name.len() as u32,
                new_text: new_name.to_owned(),
            });
        }
    }

    // All identifier references.
    for r in &index.references {
        if r.name == old_name && r.kind == ReferenceKind::Identifier {
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
