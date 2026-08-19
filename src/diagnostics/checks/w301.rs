use super::*;

// ── W301: duplicate block ─────────────────────────────────────────────────────

pub(super) fn check_w301(path: &str, index: &TemplateIndex, out: &mut Vec<Diagnostic>) {
    let mut seen: HashMap<&str, u32> = HashMap::new();
    for b in &index.blocks {
        let count = seen.entry(b.name.as_str()).or_insert(0);
        *count += 1;
        if *count >= 2 {
            out.push(Diagnostic {
                file: path.to_owned(),
                line: b.span.start_line,
                col: b.span.start_col,
                code: DiagCode::W301.code_str().to_owned(),
                slug: DiagCode::W301.slug().to_owned(),
                severity: DiagCode::W301.severity(),
                message: format!("duplicate block '{}'", b.name),
            });
        }
    }
}
