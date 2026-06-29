// REQ-DIAG-01..06, F01: check runner — pure reads over TemplateIndex/WorkspaceIndex.
// Each check emits zero or more Diagnostics; the caller applies noqa + config filters.

use std::collections::HashMap;

use crate::{
    builtins::registry::{Category, Registry},
    diagnostic::{Diagnostic, DiagnosticSeverity},
    workspace::{
        index::{ResolvedBinding, TemplateIndex, WorkspaceIndex},
        symbols::{ReferenceKind, TemplateRefKind},
    },
};

/// Run all Pass-1 (per-file) checks and return the raw findings.
///
/// Checks implemented: E001, E101, E102, E104, W201, W202, W203, W301, W302, W303, W304, W305, W402, E401, E403, E601.
pub fn run_checks(
    source: &str,
    path: &str,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    check_e001(path, index, &mut out);
    check_e101(path, index, registry, workspace, &mut out);
    check_e102_e104(path, index, registry, &mut out);
    check_w201(path, index, &mut out);
    check_w202(path, index, &mut out);
    check_w203(source, path, index, &mut out);
    check_w301(path, index, &mut out);
    check_w302(path, index, &mut out);
    check_w303(path, index, &mut out);
    check_w304(path, index, &mut out);
    check_w305(path, index, &mut out);
    check_e403(path, index, workspace, &mut out);
    check_w402_e401(source, path, index, &mut out);
    check_e601(path, index, workspace, &mut out);
    out
}

// ── E001: syntax error ────────────────────────────────────────────────────────

fn check_e001(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    for err in &index.syntax_errors {
        out.push(Diagnostic {
            file: path.to_owned(),
            line: err.span.start_line,
            col: err.span.start_col,
            code: "JINJA-E001".to_owned(),
            slug: "syntax-error".to_owned(),
            severity: DiagnosticSeverity::Error,
            message: "syntax error".to_owned(),
        });
    }
}

// ── E101: undefined-variable ──────────────────────────────────────────────────

fn check_e101(
    path: &str,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
    out: &mut Vec<Diagnostic>,
) {
    // Names that structurally suppress E101 without a registry lookup.
    let macro_names: std::collections::HashSet<&str> =
        index.macros.iter().map(|m| m.name.as_str()).collect();
    let alias_names: std::collections::HashSet<&str> =
        index.import_aliases.iter().map(|a| a.alias.as_str()).collect();
    let from_names: std::collections::HashSet<&str> = index
        .from_imports
        .iter()
        .flat_map(|fi| {
            fi.names
                .iter()
                .map(|n| n.alias.as_deref().unwrap_or(n.name.as_str()))
        })
        .collect();

    for r in &index.references {
        if r.kind != ReferenceKind::Identifier {
            continue;
        }
        // Local variable in scope — resolve_reference handles valid_range containment.
        if !matches!(index.resolve_reference(r, workspace), ResolvedBinding::HostOwned) {
            continue;
        }
        let name = r.name.as_str();
        // Local macro / import alias / from-import name.
        if macro_names.contains(name) || alias_names.contains(name) || from_names.contains(name) {
            continue;
        }
        // Jinja2 built-in global variable (loop, caller, varargs, …).
        if registry.get(Category::Variable, name).is_some() {
            continue;
        }
        // REQ-HINT-04: hinted context_variable suppresses, respecting template scope.
        if let Some(entry) = registry.get(Category::ContextVariable, name) {
            let applies = match &entry.template {
                None => true,
                Some(t) => path == t.as_str() || path.ends_with(&format!("/{t}")),
            };
            if applies {
                continue;
            }
        }
        out.push(Diagnostic {
            file: path.to_owned(),
            line: r.span.start_line,
            col: r.span.start_col,
            code: "JINJA-E101".to_owned(),
            slug: "undefined-variable".to_owned(),
            severity: DiagnosticSeverity::Error,
            message: format!("'{}' is not defined", name),
        });
    }
}

// ── E102: undefined filter / E104: undefined test ─────────────────────────────

fn check_e102_e104(path: &str, index: &TemplateIndex, registry: &Registry, out: &mut Vec<Diagnostic>) {
    for r in &index.references {
        match r.kind {
            ReferenceKind::Filter if registry.get(Category::Filter, &r.name).is_none() => {
                out.push(Diagnostic {
                    file: path.to_owned(),
                    line: r.span.start_line,
                    col: r.span.start_col,
                    code: "JINJA-E102".to_owned(),
                    slug: "undefined-filter".to_owned(),
                    severity: DiagnosticSeverity::Error,
                    message: format!("undefined filter '{}'", r.name),
                });
            }
            ReferenceKind::Test if registry.get(Category::Test, &r.name).is_none() => {
                out.push(Diagnostic {
                    file: path.to_owned(),
                    line: r.span.start_line,
                    col: r.span.start_col,
                    code: "JINJA-E104".to_owned(),
                    slug: "undefined-test".to_owned(),
                    severity: DiagnosticSeverity::Error,
                    message: format!("undefined test '{}'", r.name),
                });
            }
            _ => {}
        }
    }
}

// ── W301: duplicate block ─────────────────────────────────────────────────────

fn check_w301(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    let mut seen: HashMap<&str, u32> = HashMap::new();
    for b in &index.blocks {
        let count = seen.entry(b.name.as_str()).or_insert(0);
        *count += 1;
        if *count == 2 {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: b.span.start_line,
                col: b.span.start_col,
                code: "JINJA-W301".to_owned(),
                slug: "duplicate-block".to_owned(),
                severity: DiagnosticSeverity::Warning,
                message: format!("duplicate block '{}'", b.name),
            });
        }
    }
}

// ── W302: duplicate macro ─────────────────────────────────────────────────────

fn check_w302(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    let mut seen: HashMap<&str, u32> = HashMap::new();
    for m in &index.macros {
        let count = seen.entry(m.name.as_str()).or_insert(0);
        *count += 1;
        if *count == 2 {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: m.span.start_line,
                col: m.span.start_col,
                code: "JINJA-W302".to_owned(),
                slug: "duplicate-macro".to_owned(),
                severity: DiagnosticSeverity::Warning,
                message: format!("duplicate macro '{}'", m.name),
            });
        }
    }
}

// ── W201: unused-variable ─────────────────────────────────────────────────────

fn check_w201(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    let used_names: std::collections::HashSet<&str> =
        index.references.iter().map(|r| r.name.as_str()).collect();
    for v in &index.variables {
        // Skip variables with no valid_range (external/context vars or unpopulated bindings).
        if v.valid_range.start_byte >= v.valid_range.end_byte {
            continue;
        }
        if !used_names.contains(v.name.as_str()) {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: v.span.start_line,
                col: v.span.start_col,
                code: "JINJA-W201".to_owned(),
                slug: "unused-variable".to_owned(),
                severity: DiagnosticSeverity::Warning,
                message: format!("variable '{}' is assigned but never used", v.name),
            });
        }
    }
}

// ── W202: unused-macro ────────────────────────────────────────────────────────

fn check_w202(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    let used_names: std::collections::HashSet<&str> = index.references.iter()
        .filter(|r| matches!(r.kind, ReferenceKind::Function | ReferenceKind::Identifier))
        .map(|r| r.name.as_str())
        .collect();
    for m in &index.macros {
        if !used_names.contains(m.name.as_str()) {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: m.span.start_line,
                col: m.span.start_col,
                code: "JINJA-W202".to_owned(),
                slug: "unused-macro".to_owned(),
                severity: DiagnosticSeverity::Warning,
                message: format!("macro '{}' is defined but never called in this template", m.name),
            });
        }
    }
}

// ── W203: unused-import ───────────────────────────────────────────────────────

fn check_w203(source: &str, path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    let used_names: std::collections::HashSet<&str> =
        index.references.iter().map(|r| r.name.as_str()).collect();

    let src_bytes = source.as_bytes();

    for a in &index.import_aliases {
        // Import alias namespaces (`{% import "m" as alias %}`) are used as `alias.fn()`.
        // The attribute-access query doesn't capture them, so scan the source text directly.
        let name = a.alias.as_bytes();
        let mut found = used_names.contains(a.alias.as_str()); // already captured reference
        if !found {
            let mut pos = 0usize;
            while pos + name.len() <= src_bytes.len() {
                if &src_bytes[pos..pos + name.len()] == name {
                    let before_ok = pos == 0 || !(src_bytes[pos - 1].is_ascii_alphanumeric() || src_bytes[pos - 1] == b'_');
                    let after = pos + name.len();
                    let after_ok = after < src_bytes.len() && src_bytes[after] == b'.'; // alias.method
                    if before_ok && after_ok {
                        found = true;
                        break;
                    }
                }
                pos += 1;
            }
        }
        if !found {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: a.span.start_line,
                col: a.span.start_col,
                code: "JINJA-W203".to_owned(),
                slug: "unused-import".to_owned(),
                severity: DiagnosticSeverity::Warning,
                message: format!("import alias '{}' is never used", a.alias),
            });
        }
    }

    for fi in &index.from_imports {
        for n in &fi.names {
            let effective = n.alias.as_deref().unwrap_or(n.name.as_str());
            if !used_names.contains(effective) {
                out.push(Diagnostic {
                    file: path.to_owned(),
                    line: fi.span.start_line,
                    col: fi.span.start_col,
                    code: "JINJA-W203".to_owned(),
                    slug: "unused-import".to_owned(),
                    severity: DiagnosticSeverity::Warning,
                    message: format!("imported name '{}' is never used", effective),
                });
            }
        }
    }
}

// ── W303: duplicate-import-alias ─────────────────────────────────────────────

fn check_w303(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    let mut seen: HashMap<&str, u32> = HashMap::new();
    for a in &index.import_aliases {
        let count = seen.entry(a.alias.as_str()).or_insert(0);
        *count += 1;
        if *count == 2 {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: a.span.start_line,
                col: a.span.start_col,
                code: "JINJA-W303".to_owned(),
                slug: "duplicate-import-alias".to_owned(),
                severity: DiagnosticSeverity::Warning,
                message: format!("import alias '{}' defined more than once", a.alias),
            });
        }
    }
}

// ── W304: duplicate-from-import ───────────────────────────────────────────────

fn check_w304(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    let mut seen: HashMap<&str, u32> = HashMap::new();
    for fi in &index.from_imports {
        for n in &fi.names {
            let effective = n.alias.as_deref().unwrap_or(n.name.as_str());
            let count = seen.entry(effective).or_insert(0);
            *count += 1;
            if *count == 2 {
                out.push(Diagnostic {
                    file: path.to_owned(),
                    line: fi.span.start_line,
                    col: fi.span.start_col,
                    code: "JINJA-W304".to_owned(),
                    slug: "duplicate-from-import".to_owned(),
                    severity: DiagnosticSeverity::Warning,
                    message: format!("'{}' imported more than once", effective),
                });
            }
        }
    }
}

// ── W305: name-shadowing ──────────────────────────────────────────────────────

fn check_w305(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    let vars = &index.variables;
    for (i, inner) in vars.iter().enumerate() {
        let inner_start = inner.valid_range.start_byte;
        let inner_end = inner.valid_range.end_byte;
        if inner_start >= inner_end {
            continue;
        }
        for outer in vars[..i].iter() {
            if outer.name != inner.name {
                continue;
            }
            let outer_start = outer.valid_range.start_byte;
            let outer_end = outer.valid_range.end_byte;
            if outer_start >= outer_end {
                continue;
            }
            // Inner is nested within outer.
            if outer_start <= inner_start && inner_end <= outer_end {
                out.push(Diagnostic {
                    file: path.to_owned(),
                    line: inner.span.start_line,
                    col: inner.span.start_col,
                    code: "JINJA-W305".to_owned(),
                    slug: "name-shadowing".to_owned(),
                    severity: DiagnosticSeverity::Warning,
                    message: format!("'{}' shadows an outer-scope variable", inner.name),
                });
                break; // one diagnostic per shadowed var is enough
            }
        }
    }
}

// ── E403: missing-required-block ─────────────────────────────────────────────

fn check_e403(path: &str, index: &TemplateIndex, workspace: &WorkspaceIndex, out: &mut Vec<Diagnostic>) {
    // Only applies to child templates.
    let extends = index.template_refs.iter().find(|r| matches!(r.kind, TemplateRefKind::Extends));
    let Some(parent_ref) = extends else { return };
    let Some(parent_idx) = workspace.templates.get(&parent_ref.path) else { return };

    let child_block_names: std::collections::HashSet<&str> =
        index.blocks.iter().map(|b| b.name.as_str()).collect();

    for pb in &parent_idx.blocks {
        if pb.required && !child_block_names.contains(pb.name.as_str()) {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: 0,
                col: 0,
                code: "JINJA-E403".to_owned(),
                slug: "missing-required-block".to_owned(),
                severity: DiagnosticSeverity::Error,
                message: format!("required block '{}' is not overridden in this template", pb.name),
            });
        }
    }
}

// ── W402: unreachable-content / E401: invalid-super ──────────────────────────

fn check_w402_e401(source: &str, path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    // Only applies to child templates (those that extend a parent).
    let is_child = index.template_refs.iter().any(|r| matches!(r.kind, TemplateRefKind::Extends));
    if !is_child {
        return;
    }

    // Collect block body byte ranges ([body_start, body_end) = content between the tags).
    let block_ranges: Vec<(usize, usize)> = index.blocks.iter()
        .filter(|b| b.body.start_byte < b.body.end_byte)
        .map(|b| (b.body.start_byte, b.body.end_byte))
        .collect();

    let inside_block = |byte: usize| block_ranges.iter().any(|&(s, e)| s <= byte && byte < e);

    // W402: variables set outside any block are unreachable in a child template.
    for v in &index.variables {
        if !inside_block(v.span.start_byte) {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: v.span.start_line,
                col: v.span.start_col,
                code: "JINJA-W402".to_owned(),
                slug: "unreachable-content".to_owned(),
                severity: DiagnosticSeverity::Warning,
                message: format!("'{}' is outside any block and will not render in this extends template", v.name),
            });
        }
    }

    // E401: {{ super() }} outside any block has no parent block context.
    let needle = b"super()";
    let src_bytes = source.as_bytes();
    let mut pos = 0;
    while pos + needle.len() <= src_bytes.len() {
        if &src_bytes[pos..pos + needle.len()] == needle && !inside_block(pos) {
            let (line, col) = byte_to_line_col(source, pos);
            out.push(Diagnostic {
                file: path.to_owned(),
                line,
                col,
                code: "JINJA-E401".to_owned(),
                slug: "invalid-super".to_owned(),
                severity: DiagnosticSeverity::Error,
                message: "super() called outside a block".to_owned(),
            });
        }
        pos += 1;
    }
}

fn byte_to_line_col(source: &str, byte: usize) -> (u32, u32) {
    let byte = byte.min(source.len());
    let (mut line, mut col) = (0u32, 0u32);
    for (i, c) in source.char_indices() {
        if i >= byte { break; }
        if c == '\n' { line += 1; col = 0; } else { col += 1; }
    }
    (line, col)
}

// ── E601: template-does-not-exist ─────────────────────────────────────────────

fn check_e601(path: &str, index: &TemplateIndex, workspace: &WorkspaceIndex, out: &mut Vec<Diagnostic>) {
    for tr in &index.template_refs {
        if tr.is_dynamic || tr.ignore_missing {
            continue;
        }
        if matches!(tr.kind, TemplateRefKind::Extends | TemplateRefKind::Include | TemplateRefKind::Import | TemplateRefKind::From)
            && !workspace.templates.contains_key(&tr.path)
        {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: tr.span.start_line,
                col: tr.span.start_col,
                code: "JINJA-E601".to_owned(),
                slug: "template-does-not-exist".to_owned(),
                severity: DiagnosticSeverity::Error,
                message: format!("template '{}' does not exist", tr.path),
            });
        }
    }
}
