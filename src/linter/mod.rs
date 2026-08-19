//! REQ-FOLD-01 / F19: the `check` front-end — CLI orchestration and output
//! formatters. `main.rs` only routes; the work lives here.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::builtins::hints::{find_sidecar, load_sidecar};
use crate::builtins::registry::Registry;
use crate::config::JinjaConfig;
use crate::diagnostic::Diagnostic;
use crate::diagnostics::checks::run_checks;
use crate::diagnostics::{filter_by_config, suppress_by_noqa};
use crate::workspace::build_workspace;
use crate::workspace::index::WorkspaceIndex;

/// REQ-LINT-01..11: check command implementation.
/// Returns exit code: 0 = no findings, 1 = findings found, 2 = config/usage error.
pub fn run_check(
    paths: Vec<String>,
    format: &str,
    verbose: bool,
    config_path: Option<&str>,
    select: &[String],
    ignore: &[String],
) -> i32 {
    // REQ-LINT-03: reject slugs in --select/--ignore (must be codes or class prefixes)
    for f in select.iter().chain(ignore.iter()) {
        if !f.starts_with("JINJA-") {
            eprintln!(
                "error: invalid filter {f:?}: expected a diagnostic code or prefix (e.g. JINJA-E101, JINJA-W), not a slug"
            );
            return 2;
        }
    }

    // REQ-LINT-08: validate all explicit paths exist before doing any work
    for path_str in &paths {
        if !Path::new(path_str).exists() {
            eprintln!("error: path not found: {path_str}");
            return 2;
        }
    }

    // REQ-LINT-04/05/06: reject unknown --format values, before scanning anything.
    if !matches!(format, "rich" | "compact" | "json") {
        eprintln!("error: invalid --format value {format:?}: expected one of rich, compact, json");
        return 2;
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    let (cfg, cfg_root) = match resolve_config(config_path, &paths, &cwd) {
        Ok(pair) => pair,
        Err(code) => return code,
    };
    let ext_strs: Vec<&str> = cfg.extensions.iter().map(|s| s.as_str()).collect();

    // REQ-LINT-01: collect template dirs/files from paths
    let dirs: Vec<PathBuf> = if paths.is_empty() {
        cfg.resolved_template_dirs(&cwd)
    } else {
        paths.iter().map(|p| Path::new(p).to_path_buf()).collect()
    };

    // REQ-LINT-10: pre-canonicalize roots for path normalization
    let roots_canon: Vec<PathBuf> = dirs
        .iter()
        .map(|d| d.canonicalize().unwrap_or_else(|_| d.clone()))
        .collect();

    let dir_refs: Vec<&Path> = dirs.iter().map(|d| d.as_path()).collect();

    // REQ-LINT-09: build_workspace is the shared engine (same as LSP server)
    let t0 = std::time::Instant::now();
    let workspace = build_workspace(&dir_refs, &ext_strs);
    if verbose {
        eprintln!(
            "info: discovered {} template(s) in {:.2}s",
            workspace.templates.len(),
            t0.elapsed().as_secs_f64()
        );
    }

    // REQ-LINT-09: same registry assembly the LSP server uses, so the two
    // front-ends cannot resolve one config differently.
    let base_registry = Registry::from_config(&cfg, &cfg_root);

    let t1 = std::time::Instant::now();
    let (all_diags, source_cache) = collect_diagnostics(&workspace, &base_registry);
    if verbose {
        eprintln!(
            "info: checked {} template(s) in {:.2}s, {} raw finding(s)",
            workspace.templates.len(),
            t1.elapsed().as_secs_f64(),
            all_diags.len()
        );
    }

    // REQ-LINT-03: apply select/ignore filters (CLI overrides config; merge both)
    let mut effective_select: Vec<String> = cfg.lint.select.clone();
    effective_select.extend_from_slice(select);
    let mut effective_ignore: Vec<String> = cfg.lint.ignore.clone();
    effective_ignore.extend_from_slice(ignore);
    let sel: Vec<&str> = effective_select.iter().map(|s| s.as_str()).collect();
    let ign: Vec<&str> = effective_ignore.iter().map(|s| s.as_str()).collect();
    let filtered = filter_by_config(&all_diags, &sel, &ign);

    // REQ-DIAG-05: noqa suppression is applied AFTER select/ignore (same order as LSP server).
    let (mut sorted, w107_diags) = suppress(&filtered, &workspace, &source_cache);
    // REQ-DIAG-06/jinja-lsp-ibun: W107 (invalid-noqa) must respect the same
    // select/ignore filters as every other diagnostic code.
    sorted.extend(
        filter_by_config(&w107_diags, &sel, &ign)
            .into_iter()
            .cloned(),
    );

    // REQ-LINT-07: order by file, line, col (sort on absolute paths for stable order)
    sorted.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then(a.line.cmp(&b.line))
            .then(a.col.cmp(&b.col))
    });

    match format {
        // REQ-LINT-06/07: JSON array with 7-key shape, workspace-relative paths
        "json" => {
            let display: Vec<Diagnostic> = sorted
                .iter()
                .map(|d| Diagnostic {
                    file: normalize_path(&d.file, &roots_canon),
                    ..d.clone()
                })
                .collect();
            let json = serde_json::to_string_pretty(&display).expect("serialization must not fail");
            println!("{json}");
        }
        // REQ-LINT-05: one line per finding, 1-based line:col
        "compact" => {
            for d in &sorted {
                println!(
                    "{}:{}:{}: {} {}: {}",
                    normalize_path(&d.file, &roots_canon),
                    d.line + 1,
                    d.col + 1,
                    d.code,
                    d.slug,
                    d.message
                );
            }
        }
        // REQ-LINT-04: rustc-style report
        _ => emit_rich(&sorted, &roots_canon, &source_cache),
    }

    // REQ-LINT-08: exit codes 0 (no findings) / 1 (findings) / 2 (error)
    if sorted.is_empty() { 0 } else { 1 }
}

/// Resolve the config and the directory its relative paths (hints, templates)
/// are read from. `Err` carries the process exit code.
fn resolve_config(
    config_path: Option<&str>,
    paths: &[String],
    cwd: &Path,
) -> Result<(JinjaConfig, PathBuf), i32> {
    let parent_or = |p: &Path| {
        p.parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| cwd.to_path_buf())
    };

    if let Some(p) = config_path {
        let file = Path::new(p);
        return match JinjaConfig::from_file(file) {
            Ok(cfg) => Ok((cfg, parent_or(file))),
            Err(e) => {
                eprintln!("error: config: {e}");
                Err(2)
            }
        };
    }

    // Try CWD first; if no config is found there, also search the passed paths so
    // per-fixture jinja.toml files are respected when running `check <dir>`.
    let (cfg, found_at) = match JinjaConfig::discover_with_path(cwd) {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("error: config: {e}");
            return Err(2);
        }
    };
    if let Some(conf_path) = found_at {
        return Ok((cfg, parent_or(&conf_path)));
    }
    for path_str in paths {
        let search = Path::new(path_str);
        let search = if search.is_dir() {
            search.to_path_buf()
        } else {
            parent_or(search)
        };
        if let Ok((c, Some(conf_path))) = JinjaConfig::discover_with_path(&search) {
            return Ok((c, parent_or(&conf_path)));
        }
    }
    Ok((cfg, cwd.to_path_buf()))
}

/// Run every check over every indexed template, returning the raw findings and
/// the sources they were read from.
///
/// jinja-lsp-54gh: each template is read once and the text reused for checks,
/// noqa scanning, and rich rendering — all three used to re-read from disk, and
/// the rich formatter re-read once PER DIAGNOSTIC.
fn collect_diagnostics(
    workspace: &WorkspaceIndex,
    base_registry: &Registry,
) -> (Vec<Diagnostic>, HashMap<String, String>) {
    let mut all_diags = Vec::new();
    let mut sources = HashMap::new();
    for idx in workspace.templates.values() {
        let source = std::fs::read_to_string(&idx.path).unwrap_or_default();
        // REQ-HINT-01: overlay per-template sidecar hints on top of the base registry.
        // jinja-lsp-0zz7: only clone the base registry when a sidecar actually exists —
        // most templates have none, and the clone dwarfs the per-file check cost.
        let path = Path::new(&idx.path);
        let overlay;
        let effective_registry: &Registry = if find_sidecar(path).is_some() {
            let mut reg = base_registry.clone();
            load_sidecar(path, &mut reg);
            overlay = reg;
            &overlay
        } else {
            base_registry
        };
        all_diags.extend(run_checks(
            &source,
            &idx.path,
            idx,
            effective_registry,
            workspace,
        ));
        sources.insert(idx.path.clone(), source);
    }
    (all_diags, sources)
}

/// REQ-DIAG-05/06: apply `noqa` suppression per file, returning the surviving
/// findings and the W107s raised by invalid directives.
///
/// Every discovered template is scanned, not just files that already have
/// findings — a file whose only problem is a malformed `noqa` must still surface
/// its W107.
fn suppress(
    filtered: &[&Diagnostic],
    workspace: &WorkspaceIndex,
    sources: &HashMap<String, String>,
) -> (Vec<Diagnostic>, Vec<Diagnostic>) {
    let mut per_file: HashMap<&str, Vec<Diagnostic>> = HashMap::new();
    for d in filtered {
        per_file
            .entry(d.file.as_str())
            .or_default()
            .push((*d).clone());
    }
    let mut kept_all = Vec::new();
    let mut w107_all = Vec::new();
    let empty = String::new();
    for idx in workspace.templates.values() {
        let file_path = idx.path.as_str();
        let file_diags = per_file.remove(file_path).unwrap_or_default();
        let source = sources.get(file_path).unwrap_or(&empty);
        let (kept, w107s) = suppress_by_noqa(&file_diags, source);
        kept_all.extend(kept);
        w107_all.extend(w107s.into_iter().map(|mut d| {
            d.file = file_path.to_owned();
            d
        }));
    }
    (kept_all, w107_all)
}

/// REQ-LINT-10: absolute path to workspace-relative with forward slashes. Paths
/// outside every root are kept absolute.
///
/// The incoming path is canonicalized before comparing: on macOS `$TMPDIR`
/// resolves through a `/var -> /private/var` symlink, so an uncanonicalized
/// `/var/folders/...` path from the workspace index would never strip_prefix-match
/// a canonicalized root, silently keeping the absolute path for every finding.
fn normalize_path(abs: &str, roots: &[PathBuf]) -> String {
    let p = Path::new(abs);
    let p_canon = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    for root in roots {
        if let Ok(rel) = p_canon.strip_prefix(root) {
            return rel.to_string_lossy().replace('\\', "/");
        }
    }
    abs.replace('\\', "/")
}

/// REQ-LINT-04: rustc-style report, one block per finding.
fn emit_rich(sorted: &[Diagnostic], roots: &[PathBuf], sources: &HashMap<String, String>) {
    use std::io::IsTerminal;
    let use_color = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    let empty = String::new();
    for d in sorted {
        let source = sources.get(&d.file).unwrap_or(&empty);
        let src_line = source.lines().nth(d.line as usize).unwrap_or("");
        let display = Diagnostic {
            file: normalize_path(&d.file, roots),
            ..d.clone()
        };
        print!(
            "{}",
            format_rich_diagnostic_colored(&display, src_line, use_color)
        );
    }
    if sorted.is_empty() {
        println!("No problems found.");
    }
}

/// REQ-LINT-04: rustc-style multi-line diagnostic block, with optional ANSI color.
/// color=true: severity-colored code/caret, blue pipe/line-number; color=false: plain text.
fn format_rich_diagnostic_colored(
    d: &crate::diagnostic::Diagnostic,
    src_line: &str,
    color: bool,
) -> String {
    use crate::diagnostic::DiagnosticSeverity;
    use owo_colors::OwoColorize;

    let display_line = d.line + 1;
    let display_col = d.col + 1;

    // Apply severity color to a string slice when color is enabled.
    let sev_color = |s: &str| -> String {
        if !color {
            return s.to_owned();
        }
        match d.severity {
            DiagnosticSeverity::Error => s.red().bold().to_string(),
            DiagnosticSeverity::Warning => s.yellow().bold().to_string(),
            DiagnosticSeverity::Info => s.cyan().bold().to_string(),
            DiagnosticSeverity::Hint => s.dimmed().to_string(),
        }
    };
    let blue = |s: &str| -> String {
        if color {
            s.blue().to_string()
        } else {
            s.to_owned()
        }
    };
    let msg_styled = if color {
        d.message.bold().to_string()
    } else {
        d.message.clone()
    };

    let mut out = String::new();
    out.push_str(&format!(
        "{}: {}\n",
        sev_color(&format!("{} {}", d.code, d.slug)),
        msg_styled
    ));
    out.push_str(&format!(
        " --> {}:{}:{}\n",
        d.file, display_line, display_col
    ));

    if !src_line.is_empty() {
        let line_num = display_line.to_string();
        let pad = " ".repeat(line_num.len());
        let pipe = blue("|");
        out.push_str(&format!("{pad} {pipe}\n"));
        out.push_str(&format!("{} {pipe} {src_line}\n", blue(&line_num)));
        let col = d.col as usize;
        let after = src_line.get(col..).unwrap_or("");
        let word_len = after
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.')
            .count()
            .max(1);
        let caret = "^".repeat(word_len);
        let spaces = " ".repeat(col);
        out.push_str(&format!("{pad} {pipe} {spaces}{}\n", sev_color(&caret)));
        out.push('\n');
    }
    out
}

/// Testable no-color version for existing structural tests.
#[cfg(test)]
fn format_rich_diagnostic_for_source(d: &crate::diagnostic::Diagnostic, src_line: &str) -> String {
    format_rich_diagnostic_colored(d, src_line, false)
}

#[cfg(test)]
mod cli_tests {
    fn make_diag_with_sev(
        file: &str,
        line: u32,
        col: u32,
        code: &str,
        slug: &str,
        msg: &str,
        sev: crate::diagnostic::DiagnosticSeverity,
    ) -> crate::diagnostic::Diagnostic {
        crate::diagnostic::Diagnostic {
            file: file.to_owned(),
            line,
            col,
            code: code.to_owned(),
            slug: slug.to_owned(),
            severity: sev,
            message: msg.to_owned(),
        }
    }

    fn make_diag(
        file: &str,
        line: u32,
        col: u32,
        code: &str,
        slug: &str,
        msg: &str,
    ) -> crate::diagnostic::Diagnostic {
        use crate::diagnostic::DiagnosticSeverity;
        crate::diagnostic::Diagnostic {
            file: file.to_owned(),
            line,
            col,
            code: code.to_owned(),
            slug: slug.to_owned(),
            severity: DiagnosticSeverity::Error,
            message: msg.to_owned(),
        }
    }

    use super::*;
    #[test]
    fn jl43_rich_header_matches_spec_format() {
        let d = make_diag(
            "blog/post.html",
            3,
            8,
            "JINJA-E101",
            "undefined-variable",
            "'post.titel' is not defined",
        );
        let out = format_rich_diagnostic_for_source(&d, "{{ post.titel }}");
        assert!(
            out.starts_with("JINJA-E101 undefined-variable: 'post.titel' is not defined\n"),
            "header format must match spec"
        );
    }

    #[test]
    fn jl43_rich_location_line_is_1_based() {
        let d = make_diag(
            "blog/post.html",
            3,
            8,
            "JINJA-E101",
            "undefined-variable",
            "msg",
        );
        let out = format_rich_diagnostic_for_source(&d, "{{ post.titel }}");
        // line 3 (0-based) → display line 4; col 8 (0-based) → display col 9
        assert!(
            out.contains(" --> blog/post.html:4:9"),
            "line and col must be 1-based: {out}"
        );
    }

    #[test]
    fn jl43_rich_caret_underlines_word_at_col() {
        // Source: "{{ post.titel }}", col=8 points at 'post.titel' (10 chars)
        let d = make_diag("t.html", 0, 3, "JINJA-E101", "undefined-variable", "msg");
        let out = format_rich_diagnostic_for_source(&d, "{{ post.titel }}");
        // col=3 → after = "post.titel }}" → word = "post.titel" → 10 carets
        assert!(
            out.contains("^^^^^^^^^^"),
            "caret must underline 'post.titel' (10 chars): {out}"
        );
    }

    #[test]
    fn jl43_rich_caret_minimum_one_when_at_non_word() {
        let d = make_diag("t.html", 0, 2, "JINJA-E101", "undefined-variable", "msg");
        // col=2 → char ' ' → word_len=0, clamped to 1
        let out = format_rich_diagnostic_for_source(&d, "{{ x }}");
        assert!(out.contains('^'), "must have at least one caret: {out}");
    }

    #[test]
    fn t18_rich_no_color_produces_no_ansi_escapes() {
        let d = make_diag(
            "blog/post.html",
            0,
            3,
            "JINJA-E101",
            "undefined-variable",
            "msg",
        );
        let out = format_rich_diagnostic_colored(&d, "{{ post.titel }}", false);
        assert!(
            !out.contains(''),
            "color=false must produce no ANSI escapes: {:?}",
            out
        );
    }

    // T-17: color=true must produce ANSI escape codes for error (red)
    #[test]
    fn t17_rich_color_produces_ansi_escapes_for_error() {
        let d = make_diag(
            "blog/post.html",
            0,
            3,
            "JINJA-E101",
            "undefined-variable",
            "msg",
        );
        let out = format_rich_diagnostic_colored(&d, "{{ post.titel }}", true);
        assert!(
            out.contains(''),
            "color=true must produce ANSI escapes: {:?}",
            out
        );
    }

    // T-17: warning severity must use yellow ANSI color
    #[test]
    fn t17_rich_color_warning_uses_ansi() {
        use crate::diagnostic::DiagnosticSeverity;
        let d = make_diag_with_sev(
            "t.html",
            0,
            0,
            "JINJA-W203",
            "unused-import",
            "msg",
            DiagnosticSeverity::Warning,
        );
        let out = format_rich_diagnostic_colored(&d, "some line", true);
        assert!(
            out.contains(''),
            "warning with color=true must have ANSI escapes: {:?}",
            out
        );
    }

    // T-18: no-color output must still contain code, slug, message
    #[test]
    fn t18_no_color_output_has_code_and_message() {
        let d = make_diag(
            "blog/post.html",
            0,
            3,
            "JINJA-E101",
            "undefined-variable",
            "my message",
        );
        let out = format_rich_diagnostic_colored(&d, "line content", false);
        assert!(out.contains("JINJA-E101"), "code must be present: {out}");
        assert!(
            out.contains("undefined-variable"),
            "slug must be present: {out}"
        );
        assert!(out.contains("my message"), "message must be present: {out}");
        assert!(!out.contains(''), "must not have ANSI escapes: {:?}", out);
    }
}
