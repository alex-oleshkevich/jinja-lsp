// REQ-EXTR-09: full workspace rebuild must complete in under 2 seconds for 500 templates.

use std::{fs, time::Instant};

use jinja_lsp::workspace::build_workspace;

#[test]
fn full_rebuild_500_templates_under_2s() {
    let tmp = std::env::temp_dir().join("jinja_lsp_perf_500");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    // Generate 500 template files with varied content
    for i in 0..500 {
        let content = format!(
            "{{% block content_{i} %}}{{% endblock %}}\n\
             {{% set x_{i} = {i} %}}\n\
             {{% for item_{i} in items_{i} %}}{{% endfor %}}",
        );
        fs::write(tmp.join(format!("t{i:03}.html")), content).unwrap();
    }

    let start = Instant::now();
    let workspace = build_workspace(&[&tmp], &["html"]);
    let elapsed = start.elapsed();

    assert_eq!(workspace.templates.len(), 500, "expected 500 templates in workspace");

    // < 2s in release builds; debug builds have no optimizer so we use a
    // generous budget there. The hard requirement (< 2s) is for release mode.
    let budget = if cfg!(debug_assertions) { 15.0 } else { 2.0 };
    assert!(
        elapsed.as_secs_f64() < budget,
        "rebuild took {:.2}s, must be under {budget}s (debug={})",
        elapsed.as_secs_f64(),
        cfg!(debug_assertions),
    );
}
