use super::*;

// ── E501: wrong-call-args ─────────────────────────────────────────────────────

pub(super) fn check_e501(
    path: &str,
    index: &TemplateIndex,
    workspace: &WorkspaceIndex,
    out: &mut Vec<Diagnostic>,
) {
    for call in &index.macro_calls {
        // Resolve the callee macro definition (local → from-imports → workspace-wide).
        let Some(mac) = resolve_macro(call.callee.as_str(), index, workspace) else {
            continue;
        };

        let required_count = mac
            .parameters
            .iter()
            .filter(|p| p.default.is_none())
            .count();
        let total_count = mac.parameters.len();
        let given_positional = call.positional_count;
        let given_keywords: HashSet<&str> = call.keyword_names.iter().map(|s| s.as_str()).collect();

        // Check for unknown keyword args.
        for kw in &call.keyword_names {
            if !mac.parameters.iter().any(|p| p.name == *kw) {
                out.push(Diagnostic {
                    file: path.to_owned(),
                    line: call.span.start_line,
                    col: call.span.start_col,
                    code: DiagCode::E501.code_str().to_owned(),
                    slug: DiagCode::E501.slug().to_owned(),
                    severity: DiagCode::E501.severity(),
                    message: format!("'{}': unexpected keyword argument '{}'", call.callee, kw),
                });
            }
        }

        // Count how many required params are already satisfied by keyword args.
        let required_by_keyword = mac
            .parameters
            .iter()
            .filter(|p| p.default.is_none() && given_keywords.contains(p.name.as_str()))
            .count();
        let required_positional_needed = required_count.saturating_sub(required_by_keyword);

        // Too few positional args.
        if given_positional < required_positional_needed {
            let missing = required_positional_needed - given_positional;
            out.push(Diagnostic {
                file: path.to_owned(),
                line: call.span.start_line,
                col: call.span.start_col,
                code: DiagCode::E501.code_str().to_owned(),
                slug: DiagCode::E501.slug().to_owned(),
                severity: DiagCode::E501.severity(),
                message: format!(
                    "'{}': missing {} required argument(s) (expected at least {}, got {})",
                    call.callee, missing, required_count, given_positional
                ),
            });
            continue;
        }

        // Too many positional args.
        if given_positional > total_count {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: call.span.start_line,
                col: call.span.start_col,
                code: DiagCode::E501.code_str().to_owned(),
                slug: DiagCode::E501.slug().to_owned(),
                severity: DiagCode::E501.severity(),
                message: format!(
                    "'{}': too many positional arguments (expected at most {}, got {})",
                    call.callee, total_count, given_positional
                ),
            });
        }
    }
}

pub(super) fn resolve_macro<'a>(
    callee: &str,
    index: &'a TemplateIndex,
    workspace: &'a WorkspaceIndex,
) -> Option<&'a MacroDefinition> {
    // Local macro.
    if let Some(m) = index.macros.iter().find(|m| m.name == callee) {
        return Some(m);
    }
    // From-imports.
    for fi in &index.from_imports {
        let Some(orig) = fi
            .names
            .iter()
            .find(|n| n.alias.as_deref().unwrap_or(n.name.as_str()) == callee)
            .map(|n| n.name.as_str())
        else {
            continue;
        };
        if let Some(src_idx) = workspace.get_by_ref(&fi.source) {
            if let Some(m) = src_idx.macros.iter().find(|m| m.name == orig) {
                return Some(m);
            }
        }
    }
    // Workspace-wide.
    workspace.find_macro_workspace_wide(callee)
}
