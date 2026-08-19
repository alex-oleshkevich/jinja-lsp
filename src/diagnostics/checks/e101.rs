use super::*;

// ── E101: undefined-variable ──────────────────────────────────────────────────

pub(super) fn check_e101(
    path: &str,
    index: &TemplateIndex,
    registry: &Registry,
    workspace: &WorkspaceIndex,
    out: &mut Vec<Diagnostic>,
) {
    // F01 §10: prevent E101 cascade — tree-sitter captures filter/test names as both
    // @identifier and @filter/@custom_test.  Skip identifiers already captured precisely.
    let filter_test_bytes: std::collections::HashSet<usize> = index
        .references
        .iter()
        .filter(|r| matches!(r.kind, ReferenceKind::Filter | ReferenceKind::Test))
        .map(|r| r.span.start_byte)
        .collect();

    // Names that structurally suppress E101 without a registry lookup.
    let macro_names: std::collections::HashSet<&str> =
        index.macros.iter().map(|m| m.name.as_str()).collect();
    let alias_names: std::collections::HashSet<&str> = index
        .import_aliases
        .iter()
        .map(|a| a.alias.as_str())
        .collect();
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
        // Multi-level attribute chains (e.g. `request.user` from `{{ request.user.name }}`)
        // are captured as @object with the intermediate path as the name.  They are not
        // bare variable references and must not trigger E101.
        if r.name.contains('.') {
            continue;
        }
        // Skip identifiers that the grammar also captured precisely as a filter or test.
        if filter_test_bytes.contains(&r.span.start_byte) {
            continue;
        }
        // Local variable in scope — resolve_reference handles valid_range containment.
        if !matches!(
            index.resolve_reference(r, workspace),
            ResolvedBinding::HostOwned
        ) {
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
        // Macro parameter in scope — parameters bind within the macro body.
        let in_macro_param = index.macros.iter().any(|m| {
            m.body.start_byte < m.body.end_byte
                && m.body.start_byte <= r.span.start_byte
                && r.span.end_byte <= m.body.end_byte
                && m.parameters.iter().any(|p| p.name == name)
        });
        if in_macro_param {
            continue;
        }
        // REQ-HINT-04: hinted context_variable suppresses, respecting template scope.
        if let Some(entry) = registry.get(Category::ContextVariable, name) {
            let applies = match &entry.template {
                None => true,
                Some(t) => path_matches_template_scope(path, t),
            };
            if applies {
                continue;
            }
        }
        out.push(Diagnostic {
            file: path.to_owned(),
            line: r.span.start_line,
            col: r.span.start_col,
            code: DiagCode::E101.code_str().to_owned(),
            slug: DiagCode::E101.slug().to_owned(),
            severity: DiagCode::E101.severity(),
            message: format!("'{}' is not defined", name),
        });
    }
}
