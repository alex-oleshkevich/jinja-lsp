// REQ-DIAG-01..06, F01: check runner — pure reads over TemplateIndex/WorkspaceIndex.
// Each check emits zero or more Diagnostics; the caller applies noqa + config filters.

use std::collections::{HashMap, HashSet};

use crate::{
    builtins::registry::{Category, Registry},
    diagnostic::Diagnostic,
    diagnostics::DiagCode,
    workspace::{
        index::{ResolvedBinding, TemplateIndex, WorkspaceIndex},
        symbols::{MacroDefinition, ReferenceKind, TemplateRefKind},
    },
};

/// Run all Pass-1 (per-file) checks and return the raw findings.
///
/// Checks implemented: E001, W106, E101, E102, E103, E104, W201, W202, W203, W301, W302, W303, W304, W305, W402, E401, E403, E404, E501, E601.
pub fn run_checks(
    source: &str,
    path: &str,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    check_e001(path, index, &mut out);
    // F01 §10: when the parse has errors, only E001 fires — all other checks
    // rely on a valid AST and would produce a false-positive cascade.
    if !index.syntax_errors.is_empty() {
        return out;
    }
    check_w106(source, path, index, registry, &mut out);
    check_e101(path, index, registry, workspace, &mut out);
    check_e103(path, index, registry, workspace, &mut out);
    check_e102_e104(path, index, registry, &mut out);
    check_w201(path, index, &mut out);
    check_w202(path, index, workspace, &mut out);
    check_w203(source, path, index, &mut out);
    check_w301(path, index, &mut out);
    check_w302(path, index, &mut out);
    check_w303(path, index, &mut out);
    check_w304(path, index, &mut out);
    check_w305(path, index, &mut out);
    check_e403(path, index, workspace, &mut out);
    check_e404(path, index, workspace, &mut out);
    check_e501(path, index, workspace, &mut out);
    check_w402_e401(path, index, &mut out);
    check_e601(path, index, workspace, &mut out);
    out
}

// REQ-FOLD-04: one module per check; `run_checks` above is the only dispatcher.
mod e001;
mod e101;
mod e102_e104;
mod e103;
mod e403;
mod e404;
mod e501;
mod e601;
mod w106;
mod w201;
mod w202;
mod w203;
mod w301;
mod w302;
mod w303;
mod w304;
mod w305;
mod w402_e401;

use e001::*;
use e101::*;
use e102_e104::*;
use e103::*;
use e403::*;
use e404::*;
use e501::*;
use e601::*;
use w106::*;
use w201::*;
use w202::*;
use w203::*;
use w301::*;
use w302::*;
use w303::*;
use w304::*;
use w305::*;
use w402_e401::*;

/// REQ-HINT-03/04: true when `path` (a workspace key — a real OS path on the
/// server, or a relative Jinja key elsewhere) is the file a hinted
/// `template:`-scoped registry entry applies to. `template_ref` is always a
/// virtual, forward-slash Jinja template reference as written by the hint
/// author — never a real OS path — so `path` is normalized before comparing;
/// on Windows, `path` can contain '\\' separators that would otherwise never
/// match a "/{template_ref}" suffix check.
fn path_matches_template_scope(path: &str, template_ref: &str) -> bool {
    let normalized = path.replace('\\', "/");
    normalized == template_ref || normalized.ends_with(&format!("/{template_ref}"))
}
