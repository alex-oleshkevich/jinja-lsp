// REQ-SIG-01..05: signature help for macro, function, and filter calls.

use crate::{
    builtins::registry::{Category, Registry},
    workspace::{
        index::{TemplateIndex, WorkspaceIndex},
        symbols::MacroDefinition,
    },
};

// ── Public types ──────────────────────────────────────────────────────────────

/// One parameter slot in a signature.
#[derive(Debug, Clone)]
pub struct SignatureParam {
    pub label: String,
    pub documentation: Option<String>,
}

/// Result of a signature-help request (REQ-SIG-05).
#[derive(Debug, Clone)]
pub struct SignatureHelp {
    /// Full signature label, e.g. `truncate(s, length=255, …)`.
    pub label: String,
    pub params: Vec<SignatureParam>,
    /// `None` when the cursor is past the last declared parameter.
    pub active_parameter: Option<usize>,
}

/// Trigger characters for signature help (REQ-SIG-01).
pub const TRIGGER_CHARS: &[char] = &['(', ','];

// ── Public entry point ────────────────────────────────────────────────────────

/// Return signature help for the call the cursor sits in, or `None` when the
/// cursor is outside a call / outside Jinja delimiters (REQ-SIG-01).
pub fn signature_help(
    source: &str,
    line: u32,
    col: u32,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
) -> Option<SignatureHelp> {
    let cursor = super::line_col_to_byte(source, line, col);

    // Narrow to the text before the cursor, within the active Jinja delimiter.
    let (inner, is_filter_ctx) = jinja_inner_before(source, cursor)?;

    // Scan for the call state (enclosing open-paren and comma count).
    let state = scan_call_state(inner)?;

    // Identify the callee name and whether it's a filter call.
    let (callee, is_filter) = callee_before_paren(inner, state.open_paren_pos, is_filter_ctx)?;

    // Resolve signature from macro params, from-imports, or registry.
    let sh = resolve_signature(callee, is_filter, state.comma_count, index, registry, workspace)?;
    Some(sh)
}

// ── Delimiter detection ───────────────────────────────────────────────────────

/// Extract the inner text of the active `{{ }}` or `{% %}` delimiter, up to
/// `cursor`, and report whether the cursor is inside a render expression (`{{`).
///
/// Returns `None` when outside Jinja or inside a `{# #}` comment.
fn jinja_inner_before(source: &str, cursor: usize) -> Option<(&str, bool)> {
    let before = &source[..cursor.min(source.len())];

    let render_open = before.rfind("{{");
    let render_close = before.rfind("}}");
    let stmt_open = before.rfind("{%");
    let stmt_close = before.rfind("%}");
    let comment_open = before.rfind("{#");
    let comment_close = before.rfind("#}");

    let render_active = is_open_after_close(render_open, render_close);
    let stmt_active = is_open_after_close(stmt_open, stmt_close);
    let comment_active = is_open_after_close(comment_open, comment_close);

    if comment_active {
        return None;
    }

    let render_pos = if render_active { render_open } else { None };
    let stmt_pos = if stmt_active { stmt_open } else { None };
    let (open_pos, is_render) = match (render_pos, stmt_pos) {
        (Some(ro), Some(so)) => if ro > so { (ro, true) } else { (so, false) },
        (Some(ro), None) => (ro, true),
        (None, Some(so)) => (so, false),
        (None, None) => return None,
    };

    let inner = before.get(open_pos + 2..)?;
    Some((inner, is_render))
}

fn is_open_after_close(open: Option<usize>, close: Option<usize>) -> bool {
    match (open, close) {
        (Some(o), Some(c)) => o > c,
        (Some(_), None) => true,
        _ => false,
    }
}

// ── Call state scanning ───────────────────────────────────────────────────────

struct CallState {
    /// Byte position (within `inner`) of the enclosing `(`.
    open_paren_pos: usize,
    /// Number of top-level commas between the open paren and the end of `inner`.
    comma_count: usize,
}

/// Forward-scan `inner` text, tracking paren/bracket depth and string state,
/// to find the INNERMOST enclosing open paren and count its commas.
///
/// Uses a stack so that nested calls like `range(1, max(2, |))` return the
/// innermost call (`max`) rather than the outermost (`range`).
fn scan_call_state(inner: &str) -> Option<CallState> {
    // Stack entry: (open_paren_byte_pos, comma_count_at_this_depth).
    let mut stack: Vec<(usize, usize)> = Vec::new();
    let mut bracket_depth: i32 = 0;
    let mut brace_depth: i32 = 0;
    let mut in_string = false;
    let mut string_char = '"';
    let mut escaped = false;

    for (byte_pos, c) in inner.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            if c == '\\' {
                escaped = true;
                continue;
            }
            if c == string_char {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' | '\'' => {
                in_string = true;
                string_char = c;
            }
            '(' => stack.push((byte_pos, 0)),
            ')' => { stack.pop(); }
            '[' => bracket_depth += 1,
            ']' => bracket_depth -= 1,
            '{' => brace_depth += 1,
            '}' => brace_depth -= 1,
            ',' if !stack.is_empty() && bracket_depth == 0 && brace_depth == 0 => {
                if let Some(top) = stack.last_mut() {
                    top.1 += 1;
                }
            }
            _ => {}
        }
    }

    // Innermost unclosed paren is the stack top.
    let (open_paren_pos, comma_count) = stack.last().copied()?;
    Some(CallState { open_paren_pos, comma_count })
}

// ── Callee resolution ─────────────────────────────────────────────────────────

/// Find the callee name (identifier immediately before `paren_pos` in `inner`)
/// and determine whether this is a filter call.
///
/// Returns `(callee_name, is_filter)` or `None` if no callee is found.
fn callee_before_paren(
    inner: &str,
    paren_pos: usize,
    is_render_ctx: bool,
) -> Option<(&str, bool)> {
    let before_paren = inner.get(..paren_pos)?.trim_end();
    if before_paren.is_empty() {
        return None;
    }

    // Extract the last identifier.
    let callee_start = before_paren
        .rfind(|c: char| !c.is_alphanumeric() && c != '_')
        .map(|i| i + 1)
        .unwrap_or(0);
    let callee = &before_paren[callee_start..];
    if callee.is_empty() {
        return None;
    }

    // Determine if this is a filter call: the char before the callee (trimmed) is `|`.
    let is_filter = if is_render_ctx {
        let before_callee = before_paren[..callee_start].trim_end();
        before_callee.ends_with('|')
    } else {
        false
    };

    Some((callee, is_filter))
}

// ── Signature resolution ──────────────────────────────────────────────────────

fn resolve_signature(
    callee: &str,
    is_filter: bool,
    comma_count: usize,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
) -> Option<SignatureHelp> {
    if is_filter {
        // REQ-SIG-03: filter call — look in Category::Filter.
        let name = super::resolve_filter_alias(callee);
        let entry = registry.get(Category::Filter, name)?;
        let params: Vec<SignatureParam> = entry
            .params
            .iter()
            .map(|p| SignatureParam {
                label: registry_param_label(p),
                documentation: registry_param_doc(p),
            })
            .collect();
        // Registry params do NOT include the implicit receiver — first explicit arg is index 0.
        let raw_active = comma_count;
        let active = if raw_active < params.len() { Some(raw_active) } else { None };
        let label = build_label(callee, &params);
        return Some(SignatureHelp { label, params, active_parameter: active });
    }

    // Try macro first.
    if let Some(m) = index.macros.iter().find(|m| m.name == callee) {
        return Some(macro_signature(m, comma_count));
    }

    // Then from-imported macros.
    for fi in &index.from_imports {
        for n in &fi.names {
            let matches = n.name == callee || n.alias.as_deref() == Some(callee);
            if matches {
                if let Some(src_idx) = workspace.get_by_ref(&fi.source) {
                    if let Some(m) = src_idx.macros.iter().find(|m| m.name == n.name) {
                        return Some(macro_signature(m, comma_count));
                    }
                }
            }
        }
    }

    // Then registry functions and tests.
    let entry = [Category::Function, Category::Test]
        .iter()
        .find_map(|&cat| registry.get(cat, callee))?;
    let params: Vec<SignatureParam> = entry
        .params
        .iter()
        .map(|p| SignatureParam {
            label: registry_param_label(p),
            documentation: registry_param_doc(p),
        })
        .collect();
    let active = if comma_count < params.len() { Some(comma_count) } else { None };
    let label = entry
        .signature
        .as_deref()
        .map(str::to_owned)
        .unwrap_or_else(|| build_label(callee, &params));
    Some(SignatureHelp { label, params, active_parameter: active })
}

fn macro_signature(m: &MacroDefinition, comma_count: usize) -> SignatureHelp {
    let params: Vec<SignatureParam> = m
        .parameters
        .iter()
        .map(|p| SignatureParam {
            label: if let Some(d) = &p.default {
                format!("{}={}", p.name, d)
            } else {
                p.name.clone()
            },
            documentation: None,
        })
        .collect();
    let active = if comma_count < params.len() { Some(comma_count) } else { None };
    let label = build_label(&m.name, &params);
    SignatureHelp { label, params, active_parameter: active }
}

fn build_label(callee: &str, params: &[SignatureParam]) -> String {
    let param_labels: Vec<&str> = params.iter().map(|p| p.label.as_str()).collect();
    format!("{}({})", callee, param_labels.join(", "))
}

fn registry_param_label(p: &crate::builtins::registry::Param) -> String {
    if let Some(d) = &p.default {
        format!("{}={}", p.name, d)
    } else {
        p.name.clone()
    }
}

fn registry_param_doc(p: &crate::builtins::registry::Param) -> Option<String> {
    match (&p.ty, &p.default) {
        (Some(ty), Some(d)) => Some(format!("{ty} = {d}")),
        (Some(ty), None) => Some(ty.clone()),
        (None, Some(d)) => Some(format!("= {d}")),
        (None, None) => None,
    }
}

// ── Utilities ─────────────────────────────────────────────────────────────────

