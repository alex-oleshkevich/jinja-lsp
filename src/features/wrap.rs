// REQ-ACT-08: Wrap selection in block, if, or for.

use crate::edit::{TextEdit, WorkspaceEdit};
use std::collections::HashMap;

pub fn layer_name() -> &'static str {
    "wrap"
}

/// The kind of wrapper to insert around the selection.
#[derive(Debug, Clone)]
pub enum WrapKind {
    /// Wrap in `{% if condition %}…{% endif %}` (placeholder condition).
    If,
    /// Wrap in `{% for item in items %}…{% endfor %}` (placeholder loop).
    For,
    /// Wrap in `{% block <name> %}…{% endblock %}`.
    Block(String),
}

/// Returns `true` when the selected lines form a well-balanced set of Jinja delimiter pairs.
///
/// A selection "splits a tag" when `{%`, `{{`, or `{#` has no matching close (or vice-versa)
/// within the selected lines — inserting a wrapper around such a selection would corrupt the
/// template (P3). The check is a simple open-vs-close byte-pair count; block-level nesting
/// (e.g. unclosed `{% if %}`) is out of scope for now.
pub fn selection_is_well_formed(source: &str, start_line: u32, end_line: u32) -> bool {
    let lines: Vec<&str> = source.split('\n').collect();
    let selected = match lines.get(start_line as usize..=end_line as usize) {
        Some(sl) => sl.join("\n"),
        None => return false,
    };
    let s = selected.as_bytes();
    let count = |open: &[u8; 2], close: &[u8; 2]| -> (usize, usize) {
        let mut o = 0usize;
        let mut c = 0usize;
        for i in 0..s.len().saturating_sub(1) {
            if &s[i..i + 2] == open  { o += 1; }
            if &s[i..i + 2] == close { c += 1; }
        }
        (o, c)
    };
    let (so, sc) = count(b"{%", b"%}");
    let (eo, ec) = count(b"{{", b"}}");
    let (co, cc) = count(b"{#", b"#}");
    so == sc && eo == ec && co == cc
}

/// Produce a WorkspaceEdit that wraps [start_line, end_line] (inclusive) in the given wrapper.
///
/// Replaces the selected range with: open_tag + re-indented body (2 spaces per F18) + close_tag.
/// Host-language bytes outside the wrap are not modified (P5).
pub fn wrap_selection(
    source: &str,
    file: &str,
    start_line: u32,
    end_line: u32,
    kind: WrapKind,
) -> Option<WorkspaceEdit> {
    let lines: Vec<&str> = source.split('\n').collect();
    let body_lines = lines.get(start_line as usize..=end_line as usize)?;

    let (open_tag, close_tag) = match &kind {
        WrapKind::If => ("{% if condition %}".to_owned(), "{% endif %}".to_owned()),
        WrapKind::For => ("{% for item in items %}".to_owned(), "{% endfor %}".to_owned()),
        WrapKind::Block(name) => (
            format!("{{% block {name} %}}"),
            "{% endblock %}".to_owned(),
        ),
    };

    // Re-indent body one level (2 spaces per F18 indentation model); empty lines stay empty.
    let indented_body: String = body_lines
        .iter()
        .map(|l| if l.is_empty() { String::new() } else { format!("  {l}") })
        .collect::<Vec<_>>()
        .join("\n");

    let new_text = format!("{open_tag}\n{indented_body}\n{close_tag}");

    let end_col = lines.get(end_line as usize).map(|l| l.len() as u32).unwrap_or(0);
    let edit = TextEdit {
        start_line,
        start_col: 0,
        end_line,
        end_col,
        new_text,
    };

    let mut changes = HashMap::new();
    changes.insert(file.to_owned(), vec![edit]);
    Some(WorkspaceEdit { changes, create_files: vec![] })
}
