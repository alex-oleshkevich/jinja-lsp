// REQ-DIAG-01..06, F01: check runner — pure reads over TemplateIndex/WorkspaceIndex.
// Each check emits zero or more Diagnostics; the caller applies noqa + config filters.

use std::collections::HashMap;

use crate::{
    builtins::registry::{Category, Registry},
    diagnostic::{Diagnostic, DiagnosticSeverity},
    workspace::{
        index::{TemplateIndex, WorkspaceIndex},
        symbols::{ReferenceKind, TemplateRefKind},
    },
};

/// Run all Pass-1 (per-file) checks and return the raw findings.
///
/// Checks implemented: E001, E102, E104, W301, W302, E601.
pub fn run_checks(
    _source: &str,
    path: &str,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    check_e001(path, index, &mut out);
    check_e102_e104(path, index, registry, &mut out);
    check_w301(path, index, &mut out);
    check_w302(path, index, &mut out);
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
