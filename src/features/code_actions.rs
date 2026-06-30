// F17 — Code actions: quick-fixes derived from diagnostic catalog. REQ-ACT-01..11.

use std::collections::{HashMap, HashSet};

use crate::{
    builtins::registry::{Category, Registry},
    diagnostic::Diagnostic,
    edit::{TextEdit, WorkspaceEdit},
    features::{
        extract_macro::compute_extract_macro,
        wrap::{wrap_selection, WrapKind},
    },
    workspace::index::{TemplateIndex, WorkspaceIndex},
};

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
            "JINJA-E601" => {
                if let Some(action) = create_template(diag) {
                    actions.push(action);
                }
            }
            "JINJA-W301" => {
                if let Some(action) = remove_duplicate_block(source, file, diag, index) {
                    actions.push(action);
                }
            }
            "JINJA-W302" => {
                if let Some(action) = remove_duplicate_macro(source, file, diag, index) {
                    actions.push(action);
                }
            }
            "JINJA-W303" => {
                if let Some(action) = remove_duplicate_import_alias(source, file, diag, index) {
                    actions.push(action);
                }
            }
            "JINJA-W304" => {
                if let Some(action) = remove_duplicate_from_import(source, file, diag, index) {
                    actions.push(action);
                }
            }
            "JINJA-W305" => {
                if let Some(action) = rename_shadowing_variable(file, diag, index) {
                    actions.push(action);
                }
            }
            _ => {}
        }
    }

    actions
}

/// REQ-ACT-07 / REQ-ACT-08: Selection-triggered refactor actions (wrap + extract to macro).
pub fn selection_code_actions(
    source: &str,
    file: &str,
    start_line: u32,
    end_line: u32,
) -> Vec<CodeAction> {
    let mut actions = Vec::new();

    for (kind, title) in [
        (WrapKind::If, "Wrap selection in {% if %}"),
        (WrapKind::For, "Wrap selection in {% for %}"),
        (WrapKind::Block("new_block".to_owned()), "Wrap selection in {% block %}"),
    ] {
        if let Some(edit) = wrap_selection(source, file, start_line, end_line, kind) {
            actions.push(CodeAction {
                title: title.to_owned(),
                kind: ActionKind::RefactorRewrite,
                diagnostics: vec![],
                is_preferred: false,
                edit: Some(edit),
            });
        }
    }

    if let Some(edit) = compute_extract_macro(source, file, start_line, end_line, "extracted_macro") {
        actions.push(CodeAction {
            title: "Extract selection to macro".to_owned(),
            kind: ActionKind::RefactorExtract,
            diagnostics: vec![],
            is_preferred: false,
            edit: Some(edit),
        });
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

// ── REQ-ACT-06 — Shadowing and duplicate fixes ───────────────────────────────

fn remove_duplicate_block(
    source: &str,
    file: &str,
    diag: &Diagnostic,
    index: &TemplateIndex,
) -> Option<CodeAction> {
    let block_name = extract_quoted_name(&diag.message)?;
    let block = index.blocks.iter().find(|b| b.name == block_name && b.span.start_line == diag.line)?;
    // body.end_byte is not set for blocks; scan source for the matching {% endblock %}.
    let endblock_ln = find_endblock_line(source, block.span.start_line);
    let edit = delete_region_clean(source, block.span.start_line, endblock_ln + 1);
    Some(CodeAction {
        title: format!("Remove duplicate block '{block_name}'"),
        kind: ActionKind::QuickFix,
        diagnostics: vec![diag.clone()],
        is_preferred: true,
        edit: Some(WorkspaceEdit::single(file, edit)),
    })
}

fn remove_duplicate_macro(
    source: &str,
    file: &str,
    diag: &Diagnostic,
    index: &TemplateIndex,
) -> Option<CodeAction> {
    let macro_name = extract_quoted_name(&diag.message)?;
    let macro_def = index.macros.iter().find(|m| m.name == macro_name && m.span.start_line == diag.line)?;
    let endmacro_line = byte_to_line(source, macro_def.body.end_byte);
    let edit = delete_region_clean(source, macro_def.span.start_line, endmacro_line + 1);
    Some(CodeAction {
        title: format!("Remove duplicate macro '{macro_name}'"),
        kind: ActionKind::QuickFix,
        diagnostics: vec![diag.clone()],
        is_preferred: true,
        edit: Some(WorkspaceEdit::single(file, edit)),
    })
}

fn remove_duplicate_import_alias(
    source: &str,
    file: &str,
    diag: &Diagnostic,
    index: &TemplateIndex,
) -> Option<CodeAction> {
    let alias_name = extract_quoted_name(&diag.message)?;
    let alias = index.import_aliases.iter().find(|a| a.alias == alias_name && a.span.start_line == diag.line)?;
    let edit = delete_region_clean(source, alias.span.start_line, alias.span.start_line + 1);
    Some(CodeAction {
        title: format!("Remove duplicate import alias '{alias_name}'"),
        kind: ActionKind::QuickFix,
        diagnostics: vec![diag.clone()],
        is_preferred: true,
        edit: Some(WorkspaceEdit::single(file, edit)),
    })
}

fn remove_duplicate_from_import(
    source: &str,
    file: &str,
    diag: &Diagnostic,
    index: &TemplateIndex,
) -> Option<CodeAction> {
    let name = extract_quoted_name(&diag.message)?;
    let fi = index.from_imports.iter().find(|fi| fi.span.start_line == diag.line)?;
    let edit = delete_region_clean(source, fi.span.start_line, fi.span.start_line + 1);
    Some(CodeAction {
        title: format!("Remove duplicate from-import '{name}'"),
        kind: ActionKind::QuickFix,
        diagnostics: vec![diag.clone()],
        is_preferred: true,
        edit: Some(WorkspaceEdit::single(file, edit)),
    })
}

fn rename_shadowing_variable(
    file: &str,
    diag: &Diagnostic,
    index: &TemplateIndex,
) -> Option<CodeAction> {
    use crate::workspace::symbols::ReferenceKind;
    let var_name = extract_quoted_name(&diag.message)?;
    let new_name = format!("{var_name}_2");

    // Definition edit at the diagnostic location.
    let def_edit = TextEdit {
        start_line: diag.line,
        start_col: diag.col,
        end_line: diag.line,
        end_col: diag.col + var_name.len() as u32,
        new_text: new_name.clone(),
    };

    // Reference edits: all identifier references to this name on/after the definition line.
    // (valid_range is not populated by the extractor; use line-range as a scope heuristic.)
    let ref_edits = index.references.iter()
        .filter(|r| r.name == var_name && r.kind == ReferenceKind::Identifier && r.span.start_line >= diag.line)
        .map(|r| TextEdit {
            start_line: r.span.start_line,
            start_col: r.span.start_col,
            end_line: r.span.end_line,
            end_col: r.span.end_col,
            new_text: new_name.clone(),
        });

    let mut edits: Vec<TextEdit> = std::iter::once(def_edit).chain(ref_edits).collect();
    // Sort then dedup — definition and a grammar reference capture may share position.
    edits.sort_by_key(|e| (e.start_line, e.start_col));
    edits.dedup_by_key(|e| (e.start_line, e.start_col));

    let mut changes = HashMap::new();
    changes.insert(file.to_owned(), edits);
    Some(CodeAction {
        title: format!("Rename to `{new_name}`"),
        kind: ActionKind::QuickFix,
        diagnostics: vec![diag.clone()],
        is_preferred: true,
        edit: Some(WorkspaceEdit { changes, create_files: vec![] }),
    })
}

/// Scan source for the `{% endblock %}` line matching the block that opens at `from_line`.
fn find_endblock_line(source: &str, from_line: u32) -> u32 {
    let lines: Vec<&str> = source.split('\n').collect();
    // Single-line block: endblock on the same line as the opening tag.
    if let Some(line) = lines.get(from_line as usize) {
        if line.contains("{%") && line.contains("endblock") {
            return from_line;
        }
    }
    // Multi-line block: depth-count to find the matching endblock.
    let mut depth = 1i32;
    for (i, line) in lines.iter().enumerate().skip(from_line as usize + 1) {
        let t = line.trim();
        if t.contains("{%") && t.contains("endblock") {
            depth -= 1;
            if depth == 0 { return i as u32; }
        } else if t.contains("{%") && t.split_whitespace().any(|w| w == "block") {
            depth += 1;
        }
    }
    from_line
}

/// Delete lines [start_line, end_line) without leaving a blank line.
///
/// When start_line > 0: consumes the preceding newline instead of the following one,
/// so adjacent content stays joined. When start_line == 0: uses the standard range.
fn delete_region_clean(source: &str, start_line: u32, end_line: u32) -> TextEdit {
    let last = end_line - 1; // last line to delete (inclusive)
    if start_line > 0 {
        let prev_len = source_line(source, start_line - 1).len() as u32;
        let last_len = source_line(source, last).len() as u32;
        TextEdit { start_line: start_line - 1, start_col: prev_len, end_line: last, end_col: last_len, new_text: String::new() }
    } else {
        TextEdit { start_line, start_col: 0, end_line, end_col: 0, new_text: String::new() }
    }
}

// ── REQ-ACT-05 — Create a missing template file ──────────────────────────────

fn create_template(diag: &Diagnostic) -> Option<CodeAction> {
    let path = extract_quoted_name(&diag.message)?;
    // Reject paths that escape the templates root — defense in depth (rejected upstream too).
    if path.contains("../") || path.starts_with('/') {
        return None;
    }
    Some(CodeAction {
        title: format!("Create template `{path}`"),
        kind: ActionKind::QuickFix,
        diagnostics: vec![diag.clone()],
        is_preferred: true,
        edit: Some(WorkspaceEdit::create_file(&path)),
    })
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
