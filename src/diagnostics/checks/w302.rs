use super::*;

// ── W302: duplicate macro ─────────────────────────────────────────────────────

pub(super) fn check_w302(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    let mut seen: HashMap<&str, u32> = HashMap::new();
    for m in &index.macros {
        let count = seen.entry(m.name.as_str()).or_insert(0);
        *count += 1;
        if *count >= 2 {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: m.span.start_line,
                col: m.span.start_col,
                code: DiagCode::W302.code_str().to_owned(),
                slug: DiagCode::W302.slug().to_owned(),
                severity: DiagCode::W302.severity(),
                message: format!("duplicate macro '{}'", m.name),
            });
        }
    }
}
