// REQ-DIAG-03: filter diagnostics by select/ignore code or class prefix.

use crate::diagnostic::Diagnostic;

/// Apply `select` and `ignore` filters to `diags`.
///
/// - Empty `select` means all codes are enabled.
/// - A filter entry is a full code (`JINJA-E101`) or class prefix (`JINJA-E`).
/// - `ignore` wins over `select` when both match the same code.
pub fn filter_by_config<'a>(
    diags: &'a [Diagnostic],
    select: &[&str],
    ignore: &[&str],
) -> Vec<&'a Diagnostic> {
    diags
        .iter()
        .filter(|d| {
            // ignore wins first
            if ignore.iter().any(|f| code_matches(f, &d.code)) {
                return false;
            }
            // if select is non-empty, code must match at least one entry
            if !select.is_empty() {
                return select.iter().any(|f| code_matches(f, &d.code));
            }
            true
        })
        .collect()
}

/// Returns true if `filter` (a full code or class prefix) matches `code`.
fn code_matches(filter: &str, code: &str) -> bool {
    code.starts_with(filter)
}
