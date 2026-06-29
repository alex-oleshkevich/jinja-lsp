// F17 — Code actions: quick-fixes derived from diagnostic catalog. REQ-ACT-01..11.

use std::collections::{HashMap, HashSet};

use crate::{
    builtins::registry::{Category, Registry},
    diagnostic::Diagnostic,
    workspace::index::{TemplateIndex, WorkspaceIndex},
};

// ── Public types ──────────────────────────────────────────────────────────────

/// A line/col range (0-based) within a single file.
#[derive(Debug, Clone, PartialEq)]
pub struct TextEdit {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub new_text: String,
}

/// Per-file text edits (REQ-ACT-09).
#[derive(Debug, Clone)]
pub struct WorkspaceEdit {
    /// file → ordered list of edits (non-overlapping, top-to-bottom).
    pub changes: HashMap<String, Vec<TextEdit>>,
}

impl WorkspaceEdit {
    fn single(file: &str, edit: TextEdit) -> Self {
        let mut changes = HashMap::new();
        changes.insert(file.to_owned(), vec![edit]);
        WorkspaceEdit { changes }
    }
}

/// LSP CodeActionKind taxonomy (REQ-ACT-10).
#[derive(Debug, Clone, PartialEq)]
pub enum ActionKind {
    QuickFix,
    RefactorExtract,
    RefactorRewrite,
}

/// One entry in the lightbulb menu (REQ-ACT-09/10).
#[derive(Debug, Clone)]
pub struct CodeAction {
    pub title: String,
    pub kind: ActionKind,
    /// Triggering diagnostics (for quick-fixes).
    pub diagnostics: Vec<Diagnostic>,
    pub is_preferred: bool,
    pub edit: Option<WorkspaceEdit>,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Return all code actions applicable to the given diagnostics (REQ-ACT-01..11).
///
/// `diagnostics` should be the subset overlapping the cursor/selection range.
pub fn code_actions(
    source: &str,
    file: &str,
    diagnostics: &[Diagnostic],
    index: &TemplateIndex,
    workspace: &WorkspaceIndex,
    registry: &Registry,
) -> Vec<CodeAction> {
    let mut actions = Vec::new();

    for diag in diagnostics {
        match diag.code.as_str() {
            "JINJA-W202" => {
                if let Some(action) = remove_unused_macro(source, file, diag, index) {
                    actions.push(action);
                }
            }
            "JINJA-W203" => {
                if let Some(action) = remove_unused_import(source, file, diag, index) {
                    actions.push(action);
                }
            }
            "JINJA-E103" => {
                actions.extend(resolve_undefined_function(source, file, diag, index, workspace, registry));
            }
            "JINJA-E102" => {
                actions.extend(suggest_spelling_correction(file, diag, Category::Filter, registry));
            }
            "JINJA-E104" => {
                actions.extend(suggest_spelling_correction(file, diag, Category::Test, registry));
            }
            "JINJA-E403" => {
                if let Some(action) = insert_block_stub(source, file, diag, index) {
                    actions.push(action);
                }
            }
            _ => {}
        }
    }

    actions
}

// ── REQ-ACT-01 — Remove unused macro ─────────────────────────────────────────

fn remove_unused_macro(
    source: &str,
    file: &str,
    diag: &Diagnostic,
    index: &TemplateIndex,
) -> Option<CodeAction> {
    let macro_name = extract_quoted_name(&diag.message)?;
    let macro_def = index
        .macros
        .iter()
        .find(|m| m.name == macro_name && m.span.start_line == diag.line)?;

    // `macro_def.body.end_byte` is the start byte of the `{% endmacro %}` control tag.
    let endmacro_line = byte_to_line(source, macro_def.body.end_byte);

    let edit = TextEdit {
        start_line: macro_def.span.start_line,
        start_col: 0,
        end_line: endmacro_line + 1,
        end_col: 0,
        new_text: String::new(),
    };

    Some(CodeAction {
        title: format!("Remove unused macro '{macro_name}'"),
        kind: ActionKind::QuickFix,
        diagnostics: vec![diag.clone()],
        is_preferred: true,
        edit: Some(WorkspaceEdit::single(file, edit)),
    })
}

// ── REQ-ACT-01 — Remove unused import ────────────────────────────────────────

fn remove_unused_import(
    source: &str,
    file: &str,
    diag: &Diagnostic,
    index: &TemplateIndex,
) -> Option<CodeAction> {
    let unused_name = extract_quoted_name(&diag.message)?;

    // 1. Check ImportAlias ({% import "…" as alias %}).
    if let Some(alias) = index
        .import_aliases
        .iter()
        .find(|a| a.alias == unused_name && a.span.start_line == diag.line)
    {
        let edit = delete_whole_line(alias.span.start_line);
        return Some(CodeAction {
            title: format!("Remove unused import '{unused_name}'"),
            kind: ActionKind::QuickFix,
            diagnostics: vec![diag.clone()],
            is_preferred: true,
            edit: Some(WorkspaceEdit::single(file, edit)),
        });
    }

    // 2. Check FromImport ({% from "…" import a, b, … %}).
    let from_import = index.from_imports.iter().find(|fi| {
        fi.span.start_line == diag.line
            && fi.names.iter().any(|n| {
                n.name == unused_name || n.alias.as_deref() == Some(&unused_name)
            })
    })?;

    // Single-name import → delete the whole line.
    if from_import.names.len() == 1 {
        let edit = delete_whole_line(from_import.span.start_line);
        return Some(CodeAction {
            title: format!("Remove unused import '{unused_name}'"),
            kind: ActionKind::QuickFix,
            diagnostics: vec![diag.clone()],
            is_preferred: true,
            edit: Some(WorkspaceEdit::single(file, edit)),
        });
    }

    // Multi-name from-import: remove only the unused name + adjacent separator/alias.
    let line_idx = from_import.span.start_line;
    let line = source_line(source, line_idx);
    let new_line = remove_name_from_import_line(line, &unused_name)?;
    let edit = TextEdit {
        start_line: line_idx,
        start_col: 0,
        end_line: line_idx,
        end_col: line.len() as u32,
        new_text: new_line,
    };
    Some(CodeAction {
        title: format!("Remove unused import '{unused_name}'"),
        kind: ActionKind::QuickFix,
        diagnostics: vec![diag.clone()],
        is_preferred: true,
        edit: Some(WorkspaceEdit::single(file, edit)),
    })
}

// ── REQ-ACT-02 — Resolve undefined functions ──────────────────────────────────

fn resolve_undefined_function(
    _source: &str,
    file: &str,
    diag: &Diagnostic,
    index: &TemplateIndex,
    workspace: &WorkspaceIndex,
    registry: &Registry,
) -> Vec<CodeAction> {
    let Some(undef_name) = extract_quoted_name(&diag.message) else {
        return vec![];
    };

    let mut actions = Vec::new();

    // Exact workspace match → import fix (isPreferred).
    if let Some(macro_path) = find_macro_in_workspace(workspace, &undef_name) {
        let edit = import_text_edit(index, &macro_path, &undef_name);
        actions.push(CodeAction {
            title: format!("Import `{undef_name}` from \"{macro_path}\""),
            kind: ActionKind::QuickFix,
            diagnostics: vec![diag.clone()],
            is_preferred: true,
            edit: Some(WorkspaceEdit::single(file, edit)),
        });
    }

    // Near-matches → optional import fix + "Did you mean?" (REQ-ACT-02 §T-05).
    let threshold = edit_distance_threshold(&undef_name);
    let candidates = near_matches(&undef_name, threshold, workspace, registry);

    for candidate in &candidates {
        // For near-match workspace macros, also offer an import fix (not preferred).
        if let Some(macro_path) = find_macro_in_workspace(workspace, candidate) {
            let edit = import_text_edit(index, &macro_path, candidate);
            actions.push(CodeAction {
                title: format!("Import `{candidate}` from \"{macro_path}\""),
                kind: ActionKind::QuickFix,
                diagnostics: vec![diag.clone()],
                is_preferred: false,
                edit: Some(WorkspaceEdit::single(file, edit)),
            });
        }
        let edit = TextEdit {
            start_line: diag.line,
            start_col: diag.col,
            end_line: diag.line,
            end_col: diag.col + undef_name.len() as u32,
            new_text: candidate.clone(),
        };
        actions.push(CodeAction {
            title: format!("Did you mean `{candidate}`?"),
            kind: ActionKind::QuickFix,
            diagnostics: vec![diag.clone()],
            is_preferred: false,
            edit: Some(WorkspaceEdit::single(file, edit)),
        });
    }

    actions
}

// ── REQ-ACT-03 — Suggest corrections for undefined filters / tests ────────────

// Registry-only search; workspace macros are not filters/tests.
fn suggest_spelling_correction(
    file: &str,
    diag: &Diagnostic,
    category: Category,
    registry: &Registry,
) -> Vec<CodeAction> {
    let Some(misspelled) = extract_quoted_name(&diag.message) else {
        return vec![];
    };
    let threshold = edit_distance_threshold(&misspelled);
    let mut candidates: Vec<(usize, String)> = registry
        .iter_by_category(category)
        .into_iter()
        .filter_map(|e| {
            if e.name == misspelled { return None; }
            let d = levenshtein(&misspelled, &e.name);
            if d <= threshold { Some((d, e.name.clone())) } else { None }
        })
        .collect();

    candidates.sort_by_key(|(d, _)| *d);
    let mut seen = HashSet::new();
    candidates.retain(|(_, n)| seen.insert(n.clone()));

    candidates
        .into_iter()
        .map(|(_, candidate)| {
            let edit = TextEdit {
                start_line: diag.line,
                start_col: diag.col,
                end_line: diag.line,
                end_col: diag.col + misspelled.len() as u32,
                new_text: candidate.clone(),
            };
            CodeAction {
                title: format!("Did you mean `{candidate}`?"),
                kind: ActionKind::QuickFix,
                diagnostics: vec![diag.clone()],
                is_preferred: false,
                edit: Some(WorkspaceEdit::single(file, edit)),
            }
        })
        .collect()
}

// ── REQ-ACT-04 — Insert a stub for a missing required block ──────────────────

fn insert_block_stub(
    source: &str,
    file: &str,
    diag: &Diagnostic,
    index: &TemplateIndex,
) -> Option<CodeAction> {
    let block_name = extract_quoted_name(&diag.message)?;
    let extends_ln = extends_line(index)?;
    let line_str = source_line(source, extends_ln);
    let indent_len = line_str.len() - line_str.trim_start().len();
    let indent = &line_str[..indent_len];
    let insert_ln = extends_ln + 1;
    let new_text = format!("{indent}{{% block {block_name} %}}{{% endblock %}}\n");
    let edit = TextEdit {
        start_line: insert_ln,
        start_col: 0,
        end_line: insert_ln,
        end_col: 0,
        new_text,
    };
    Some(CodeAction {
        title: format!("Insert `{block_name}` block"),
        kind: ActionKind::QuickFix,
        diagnostics: vec![diag.clone()],
        is_preferred: true,
        edit: Some(WorkspaceEdit::single(file, edit)),
    })
}

/// Return the 0-based line of the extends tag, if any.
fn extends_line(index: &TemplateIndex) -> Option<u32> {
    index.extends().map(|r| r.span.start_line)
}

/// Build a TextEdit that inserts an import line after extends (or at top).
fn import_text_edit(index: &TemplateIndex, macro_path: &str, macro_name: &str) -> TextEdit {
    let insert_line = extends_line(index).map(|l| l + 1).unwrap_or(0);
    TextEdit {
        start_line: insert_line,
        start_col: 0,
        end_line: insert_line,
        end_col: 0,
        new_text: format!("{{% from \"{macro_path}\" import {macro_name} %}}\n"),
    }
}

/// Search all workspace templates for a macro named `name`; return its template path.
fn find_macro_in_workspace(workspace: &WorkspaceIndex, name: &str) -> Option<String> {
    workspace.templates.iter().find_map(|(path, idx)| {
        idx.macros.iter().any(|m| m.name == name).then(|| path.clone())
    })
}

/// Edit-distance threshold for near-match suggestions.
fn edit_distance_threshold(name: &str) -> usize {
    // Allow 1 edit for names up to 5 chars, 2 for longer names — avoids false positives.
    if name.len() <= 5 { 1 } else { 2 }
}

/// Collect names within `threshold` edit distance from `name`, excluding exact match.
fn near_matches(
    name: &str,
    threshold: usize,
    workspace: &WorkspaceIndex,
    registry: &Registry,
) -> Vec<String> {
    let mut results: Vec<(usize, String)> = Vec::new();

    // Workspace macros.
    for idx in workspace.templates.values() {
        for m in &idx.macros {
            if m.name != name {
                let d = levenshtein(name, &m.name);
                if d <= threshold {
                    results.push((d, m.name.clone()));
                }
            }
        }
    }

    // Registry functions/globals.
    for entry in registry.iter_by_category(Category::Function) {
        if entry.name != name {
            let d = levenshtein(name, &entry.name);
            if d <= threshold {
                results.push((d, entry.name.clone()));
            }
        }
    }

    results.sort_by_key(|(d, _)| *d);
    // Deduplicate by name (a name can appear in both workspace and registry).
    let mut seen = HashSet::new();
    results.retain(|(_, n)| seen.insert(n.clone()));
    results.into_iter().map(|(_, n)| n).collect()
}

/// Standard Levenshtein edit distance.
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let m = a.len();
    let n = b.len();
    if m == 0 { return n; }
    if n == 0 { return m; }
    // Use two rows to keep O(n) space.
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Extract the name from messages like "unused macro 'foo'" or "unused import 'bar'".
fn extract_quoted_name(message: &str) -> Option<String> {
    let start = message.find('\'')?;
    let rest = &message[start + 1..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_owned())
}

/// Return 0-based line number for the given byte offset.
fn byte_to_line(source: &str, byte: usize) -> u32 {
    source[..byte.min(source.len())].bytes().filter(|&b| b == b'\n').count() as u32
}

/// Return the source line (without trailing newline) at 0-based `line`.
fn source_line(source: &str, line: u32) -> &str {
    source.split('\n').nth(line as usize).unwrap_or("")
}

/// A whole-line delete: replaces `[line, 0) .. [line+1, 0)` with "".
fn delete_whole_line(line: u32) -> TextEdit {
    TextEdit {
        start_line: line,
        start_col: 0,
        end_line: line + 1,
        end_col: 0,
        new_text: String::new(),
    }
}

/// Remove `name` (and its adjacent `as alias` and comma/space separator) from an import line.
///
/// Example: `{% from "x.html" import a, b as bb %}` removing `bb`
///       →  `{% from "x.html" import a %}`
fn remove_name_from_import_line(line: &str, name: &str) -> Option<String> {
    let import_kw = line.find("import ")?;
    let after_import = import_kw + "import ".len();
    let close = line.rfind("%}")?;
    let names_section = line[after_import..close].trim_end();

    let parts: Vec<&str> = names_section.split(',').map(|s| s.trim()).collect();
    let kept: Vec<&str> = parts
        .iter()
        .copied()
        .filter(|entry| {
            let words: Vec<&str> = entry.split_whitespace().collect();
            match words.as_slice() {
                [n] => *n != name,
                [n, "as", alias] => *n != name && *alias != name,
                _ => true,
            }
        })
        .collect();

    let new_names = kept.join(", ");
    let prefix = &line[..after_import];
    let suffix = &line[close..]; // includes "%}"
    Some(format!("{prefix}{new_names} {suffix}"))
}
