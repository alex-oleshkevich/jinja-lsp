use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "jinja-lsp", about = "Jinja2 template language server", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the LSP server over stdio (default)
    Lsp,
    /// Check templates for diagnostics (REQ-LINT-01..11)
    Check {
        /// Files or directories to check (optional; defaults to configured templates dirs)
        paths: Vec<String>,
        /// Output format: rich (default), compact, json
        #[arg(long, default_value = "rich")]
        format: String,
        /// Enable verbose output on stderr
        #[arg(long, short)]
        verbose: bool,
        /// Path to config file (overrides discovery)
        #[arg(long)]
        config: Option<String>,
        /// Enable only these diagnostic codes/prefixes
        #[arg(long, value_delimiter = ',')]
        select: Vec<String>,
        /// Disable these diagnostic codes/prefixes
        #[arg(long, value_delimiter = ',')]
        ignore: Vec<String>,
    },
    /// Format Jinja templates in place (or --check / --diff read-only)
    Format {
        /// Files or directories to format (optional; defaults to templates/)
        paths: Vec<String>,
        /// Path to config file (overrides discovery)
        #[arg(long)]
        config: Option<String>,
        /// Check only — do not write, exit 1 if any file would change
        #[arg(long)]
        check: bool,
        /// Print unified diff — do not write, exit 1 if any file would change
        #[arg(long)]
        diff: bool,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let code = match cli.command.unwrap_or(Commands::Lsp) {
        Commands::Lsp => {
            jinja_lsp::server::run_lsp_server().await;
            0
        }
        Commands::Check { paths, format, verbose, config, select, ignore } => {
            run_check(paths, &format, verbose, config.as_deref(), &select, &ignore)
        }
        Commands::Format { paths, config, check, diff } => {
            run_format(paths, config.as_deref(), check, diff)
        }
    };
    std::process::exit(code);
}

/// REQ-LINT-01..11: check command implementation.
/// Returns exit code: 0 = no findings, 1 = findings found, 2 = config/usage error.
fn run_check(paths: Vec<String>, format: &str, verbose: bool, config_path: Option<&str>, select: &[String], ignore: &[String]) -> i32 {
    use std::path::Path;
    use jinja_lsp::config::JinjaConfig;
    use jinja_lsp::diagnostic::Diagnostic;
    use jinja_lsp::diagnostics::filter_by_config;
    use jinja_lsp::workspace::build_workspace;

    // REQ-LINT-03: reject slugs in --select/--ignore (must be codes or class prefixes)
    for f in select.iter().chain(ignore.iter()) {
        if !f.starts_with("JINJA-") {
            eprintln!("error: invalid filter {f:?}: expected a diagnostic code or prefix (e.g. JINJA-E101, JINJA-W), not a slug");
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

    let cwd = std::env::current_dir().unwrap_or_default();
    // cfg_root is the directory that relative config paths (hints, templates) are resolved from.
    let (cfg, cfg_root) = match config_path {
        Some(p) => {
            let file = Path::new(p);
            let root = file.parent().map(|d| d.to_owned()).unwrap_or_else(|| cwd.clone());
            match JinjaConfig::from_file(file) {
                Ok(c) => (c, root),
                Err(e) => { eprintln!("error: config: {e}"); return 2; }
            }
        }
        None => {
            // Try CWD first; if no config found, also search the passed paths so
            // per-fixture jinja.toml files are respected when running `check <dir>`.
            let (cfg, found_at) = match JinjaConfig::discover_with_path(&cwd) {
                Ok(pair) => pair,
                Err(e) => { eprintln!("error: config: {e}"); return 2; }
            };
            if let Some(ref conf_path) = found_at {
                let root = conf_path.parent().map(|d| d.to_owned()).unwrap_or_else(|| cwd.clone());
                (cfg, root)
            } else if paths.is_empty() {
                (cfg, cwd.clone())
            } else {
                // CWD had no config; search each provided path.
                let mut result = (cfg, cwd.clone());
                for path_str in &paths {
                    let search = Path::new(path_str);
                    let search = if search.is_dir() { search.to_owned() } else {
                        search.parent().map(|p| p.to_owned()).unwrap_or_else(|| cwd.clone())
                    };
                    if let Ok((c, Some(conf_path))) = JinjaConfig::discover_with_path(&search) {
                        let root = conf_path.parent().map(|d| d.to_owned()).unwrap_or_else(|| cwd.clone());
                        result = (c, root);
                        break;
                    }
                }
                result
            }
        }
    };
    let ext_strs: Vec<&str> = cfg.extensions.iter().map(|s| s.as_str()).collect();

    // REQ-LINT-01: collect template dirs/files from paths
    let dirs: Vec<std::path::PathBuf> = if paths.is_empty() {
        cfg.resolved_template_dirs(&cwd)
    } else {
        paths.iter().map(|p| Path::new(p).to_path_buf()).collect()
    };

    // REQ-LINT-10: pre-canonicalize roots for path normalization
    let roots_canon: Vec<std::path::PathBuf> = dirs.iter()
        .map(|d| d.canonicalize().unwrap_or_else(|_| d.clone()))
        .collect();

    let dir_refs: Vec<&Path> = dirs.iter().map(|d| d.as_path()).collect();

    // REQ-LINT-09: build_workspace is the shared engine (same as LSP server)
    let t0 = std::time::Instant::now();
    let workspace = build_workspace(&dir_refs, &ext_strs);
    if verbose {
        eprintln!("info: discovered {} template(s) in {:.2}s",
            workspace.templates.len(), t0.elapsed().as_secs_f64());
    }

    // REQ-LINT-09: run all per-file checks across every indexed template.
    use jinja_lsp::builtins::registry::Registry;
    use jinja_lsp::diagnostics::checks::run_checks;
    use jinja_lsp::diagnostics::suppress_by_noqa;
    // Build registry the same way the LSP server does — core + extras + custom_builtins + hints.
    // Relative paths are resolved against cfg_root (the directory containing jinja.toml).
    let mut registry = Registry::load_core();
    let extras: Vec<&str> = cfg.extras.iter().map(|s| s.as_str()).collect();
    registry.load_packs(&extras);
    for dir_str in &cfg.custom_builtins {
        registry.load_custom_builtins(&cfg_root.join(dir_str));
    }
    for dir_str in &cfg.hints {
        registry.load_hints_from_dir(&cfg_root.join(dir_str));
    }

    let t1 = std::time::Instant::now();
    let mut all_diags: Vec<Diagnostic> = Vec::new();
    for idx in workspace.templates.values() {
        let source = std::fs::read_to_string(&idx.path).unwrap_or_default();
        let raw = run_checks(&source, &idx.path, idx, &registry, &workspace);
        let (kept, w107s) = suppress_by_noqa(&raw, &source);
        all_diags.extend(kept);
        all_diags.extend(w107s);
    }
    if verbose {
        eprintln!("info: checked {} template(s) in {:.2}s, {} raw finding(s)",
            workspace.templates.len(), t1.elapsed().as_secs_f64(), all_diags.len());
    }

    // REQ-LINT-03: apply select/ignore filters (CLI overrides config; merge both)
    let mut effective_select: Vec<String> = cfg.lint.select.clone();
    effective_select.extend_from_slice(select);
    let mut effective_ignore: Vec<String> = cfg.lint.ignore.clone();
    effective_ignore.extend_from_slice(ignore);
    let sel: Vec<&str> = effective_select.iter().map(|s| s.as_str()).collect();
    let ign: Vec<&str> = effective_ignore.iter().map(|s| s.as_str()).collect();
    let filtered = filter_by_config(&all_diags, &sel, &ign);

    // REQ-LINT-07: order by file, line, col (sort on absolute paths for stable order)
    let mut sorted: Vec<Diagnostic> = filtered.into_iter().cloned().collect();
    sorted.sort_by(|a, b| {
        a.file.cmp(&b.file)
            .then(a.line.cmp(&b.line))
            .then(a.col.cmp(&b.col))
    });

    // REQ-LINT-10: normalize absolute file path to workspace-relative with forward slashes.
    // For files outside all roots the absolute path is kept (as-is).
    let normalize_path = |abs: &str| -> String {
        let p = Path::new(abs);
        for root in &roots_canon {
            if let Ok(rel) = p.strip_prefix(root) {
                return rel.to_string_lossy().replace('\\', "/");
            }
        }
        abs.replace('\\', "/")
    };

    // REQ-LINT-04/05/06: output format — reject unknown values (exit 2).
    if !matches!(format, "rich" | "compact" | "json") {
        eprintln!("error: invalid --format value {:?}: expected one of rich, compact, json", format);
        return 2;
    }
    match format {
        "json" => {
            // REQ-LINT-06/07: JSON array with 7-key shape, workspace-relative paths
            let display: Vec<Diagnostic> = sorted.iter()
                .map(|d| Diagnostic { file: normalize_path(&d.file), ..d.clone() })
                .collect();
            let json = serde_json::to_string_pretty(&display).expect("serialization must not fail");
            println!("{json}");
        }
        "compact" => {
            // REQ-LINT-05: one line per finding, 1-based line:col
            for d in &sorted {
                println!("{}:{}:{}: {} {}: {}",
                    normalize_path(&d.file), d.line + 1, d.col + 1,
                    d.code, d.slug, d.message);
            }
        }
        _ => {
            // REQ-LINT-04: rich rustc-style report
            use std::io::IsTerminal;
            let use_color = std::io::stdout().is_terminal()
                && std::env::var_os("NO_COLOR").is_none();
            for d in &sorted {
                let display_path = normalize_path(&d.file);
                let source = std::fs::read_to_string(&d.file).unwrap_or_default();
                let src_line = source.lines().nth(d.line as usize).unwrap_or("");
                let display_d = Diagnostic { file: display_path, ..d.clone() };
                print!("{}", format_rich_diagnostic_colored(&display_d, src_line, use_color));
            }
            if sorted.is_empty() {
                println!("No problems found.");
            }
        }
    }

    // REQ-LINT-08: exit codes 0 (no findings) / 1 (findings) / 2 (error)
    if sorted.is_empty() { 0 } else { 1 }
}

/// REQ-FMT-08 / REQ-FMT-09: format command.
/// Returns exit code: 0 = nothing changed, 1 = changed (or would), 2 = I/O error.
fn run_format(paths: Vec<String>, config_path: Option<&str>, check: bool, diff: bool) -> i32 {
    use std::path::Path;
    use jinja_lsp::config::JinjaConfig;

    let cwd = std::env::current_dir().unwrap_or_default();
    let cfg = match config_path {
        Some(p) => match JinjaConfig::from_file(Path::new(p)) {
            Ok(c) => c,
            Err(e) => { eprintln!("error: config: {e}"); return 2; }
        },
        None => match JinjaConfig::discover(&cwd) {
            Ok(c) => c,
            Err(e) => { eprintln!("error: config: {e}"); return 2; }
        },
    };
    let ext_strs: Vec<&str> = cfg.extensions.iter().map(|s| s.as_str()).collect();
    let template_exts: &[&str] = &ext_strs;

    let roots: Vec<std::path::PathBuf> = if paths.is_empty() {
        cfg.resolved_template_dirs(&cwd)
    } else {
        paths.iter().map(|p| Path::new(p).to_path_buf()).collect()
    };

    // Collect all template files from roots.
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    for root in &roots {
        if root.is_file() {
            if let Some(ext) = root.extension().and_then(|e| e.to_str()) {
                if template_exts.contains(&ext) {
                    files.push(root.clone());
                } else {
                    // Single file with non-template ext is a no-op.
                }
            } else {
                files.push(root.clone());
            }
        } else if root.is_dir() {
            collect_template_files(root, template_exts, &mut files);
        }
    }

    let mut changed_count: usize = 0;
    let mut unchanged_count: usize = 0;

    for path in &files {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: {}: {e}", path.display());
                return 2;
            }
        };

        let formatted = jinja_lsp::format::format(&source);
        if formatted == source {
            unchanged_count += 1;
            continue;
        }

        changed_count += 1;

        if check {
            // REQ-FMT-08: per-file "would reformat" line in --check mode.
            println!("would reformat: {}", path.display());
        }

        if diff {
            print_unified_diff(path, &source, &formatted);
        }

        if !check && !diff {
            if let Err(e) = std::fs::write(path, formatted.as_bytes()) {
                eprintln!("error: {}: {e}", path.display());
                return 2;
            }
        }
    }

    // REQ-FMT-08: summary line for --check and --diff modes.
    if check || diff {
        let f = if changed_count == 1 { "file" } else { "files" };
        if check {
            println!("{changed_count} {f} would be reformatted, {unchanged_count} unchanged.");
        } else {
            // diff mode only shows changed count.
            println!("{changed_count} {f} would be reformatted.");
        }
    }

    if changed_count > 0 { 1 } else { 0 }
}

#[cfg(test)]
mod cli_tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn vn6f_insertion_shows_correct_hunk() {
        // A real unified diff should show the inserted line with + prefix and
        // proper @@ hunk coordinates — NOT cascade every subsequent line as changed.
        let original = "line1\nline2\nline3\n";
        let formatted = "line1\nnew_line\nline2\nline3\n";
        let out = capture_unified_diff(Path::new("t.html"), original, formatted);
        assert!(out.contains("@@ -1,"), "must have hunk header");
        assert!(out.contains("+new_line"), "inserted line must appear with +");
        assert!(out.contains(" line2"), "unchanged line2 must appear as context");
        assert!(out.contains(" line3"), "unchanged line3 must appear as context");
        // The naive impl would have shown -line2, +new_line, -line3, +line2 — check that doesn't happen.
        assert!(!out.contains("-line2\n+new_line"), "must not misalign existing lines as deletions");
    }

    #[test]
    fn vn6f_deletion_shows_correct_hunk() {
        let original = "line1\nline2\nline3\n";
        let formatted = "line1\nline3\n";
        let out = capture_unified_diff(Path::new("t.html"), original, formatted);
        assert!(out.contains("-line2"), "deleted line must appear with -");
        assert!(out.contains(" line3"), "unchanged line3 must appear as context, not as changed");
    }

    #[test]
    fn vn6f_identical_files_produce_no_hunks() {
        let src = "a\nb\nc\n";
        let out = capture_unified_diff(Path::new("t.html"), src, src);
        assert!(!out.contains("@@"), "identical files must produce no hunks");
    }

    // REQ-LINT-04: rich format tests
    fn make_diag(file: &str, line: u32, col: u32, code: &str, slug: &str, msg: &str) -> jinja_lsp::diagnostic::Diagnostic {
        use jinja_lsp::diagnostic::DiagnosticSeverity;
        jinja_lsp::diagnostic::Diagnostic {
            file: file.to_owned(), line, col,
            code: code.to_owned(), slug: slug.to_owned(),
            severity: DiagnosticSeverity::Error,
            message: msg.to_owned(),
        }
    }

    #[test]
    fn jl43_rich_header_matches_spec_format() {
        let d = make_diag("blog/post.html", 3, 8, "JINJA-E101", "undefined-variable", "'post.titel' is not defined");
        let out = format_rich_diagnostic_for_source(&d, "{{ post.titel }}");
        assert!(out.starts_with("JINJA-E101 undefined-variable: 'post.titel' is not defined\n"), "header format must match spec");
    }

    #[test]
    fn jl43_rich_location_line_is_1_based() {
        let d = make_diag("blog/post.html", 3, 8, "JINJA-E101", "undefined-variable", "msg");
        let out = format_rich_diagnostic_for_source(&d, "{{ post.titel }}");
        // line 3 (0-based) → display line 4; col 8 (0-based) → display col 9
        assert!(out.contains(" --> blog/post.html:4:9"), "line and col must be 1-based: {out}");
    }

    #[test]
    fn jl43_rich_caret_underlines_word_at_col() {
        // Source: "{{ post.titel }}", col=8 points at 'post.titel' (10 chars)
        let d = make_diag("t.html", 0, 3, "JINJA-E101", "undefined-variable", "msg");
        let out = format_rich_diagnostic_for_source(&d, "{{ post.titel }}");
        // col=3 → after = "post.titel }}" → word = "post.titel" → 10 carets
        assert!(out.contains("^^^^^^^^^^"), "caret must underline 'post.titel' (10 chars): {out}");
    }

    #[test]
    fn jl43_rich_caret_minimum_one_when_at_non_word() {
        let d = make_diag("t.html", 0, 2, "JINJA-E101", "undefined-variable", "msg");
        // col=2 → char ' ' → word_len=0, clamped to 1
        let out = format_rich_diagnostic_for_source(&d, "{{ x }}");
        assert!(out.contains('^'), "must have at least one caret: {out}");
    }

    #[test]
    fn vn6f_diff_header_matches_spec() {
        let out = capture_unified_diff(
            Path::new("templates/blog/post.html"),
            "{%if%}\n",
            "{% if %}\n",
        );
        assert!(out.starts_with("--- templates/blog/post.html\n"), "--- header must match spec");
        assert!(out.contains("+++ templates/blog/post.html (formatted)\n"), "+++ header must match spec");
    }

    fn make_diag_with_sev(file: &str, line: u32, col: u32, code: &str, slug: &str, msg: &str, sev: jinja_lsp::diagnostic::DiagnosticSeverity) -> jinja_lsp::diagnostic::Diagnostic {
        jinja_lsp::diagnostic::Diagnostic {
            file: file.to_owned(), line, col,
            code: code.to_owned(), slug: slug.to_owned(),
            severity: sev,
            message: msg.to_owned(),
        }
    }

    // T-18/T-19: color=false must produce no ANSI escape codes
    #[test]
    fn t18_rich_no_color_produces_no_ansi_escapes() {
        let d = make_diag("blog/post.html", 0, 3, "JINJA-E101", "undefined-variable", "msg");
        let out = format_rich_diagnostic_colored(&d, "{{ post.titel }}", false);
        assert!(!out.contains(''), "color=false must produce no ANSI escapes: {:?}", out);
    }

    // T-17: color=true must produce ANSI escape codes for error (red)
    #[test]
    fn t17_rich_color_produces_ansi_escapes_for_error() {
        let d = make_diag("blog/post.html", 0, 3, "JINJA-E101", "undefined-variable", "msg");
        let out = format_rich_diagnostic_colored(&d, "{{ post.titel }}", true);
        assert!(out.contains(''), "color=true must produce ANSI escapes: {:?}", out);
    }

    // T-17: warning severity must use yellow ANSI color
    #[test]
    fn t17_rich_color_warning_uses_ansi() {
        use jinja_lsp::diagnostic::DiagnosticSeverity;
        let d = make_diag_with_sev("t.html", 0, 0, "JINJA-W203", "unused-import", "msg", DiagnosticSeverity::Warning);
        let out = format_rich_diagnostic_colored(&d, "some line", true);
        assert!(out.contains(''), "warning with color=true must have ANSI escapes: {:?}", out);
    }

    // T-18: no-color output must still contain code, slug, message
    #[test]
    fn t18_no_color_output_has_code_and_message() {
        let d = make_diag("blog/post.html", 0, 3, "JINJA-E101", "undefined-variable", "my message");
        let out = format_rich_diagnostic_colored(&d, "line content", false);
        assert!(out.contains("JINJA-E101"), "code must be present: {out}");
        assert!(out.contains("undefined-variable"), "slug must be present: {out}");
        assert!(out.contains("my message"), "message must be present: {out}");
        assert!(!out.contains(''), "must not have ANSI escapes: {:?}", out);
    }
}

/// REQ-LINT-04: rustc-style multi-line diagnostic block, with optional ANSI color.
/// color=true: severity-colored code/caret, blue pipe/line-number; color=false: plain text.
fn format_rich_diagnostic_colored(
    d: &jinja_lsp::diagnostic::Diagnostic,
    src_line: &str,
    color: bool,
) -> String {
    use jinja_lsp::diagnostic::DiagnosticSeverity;
    use owo_colors::OwoColorize;

    let display_line = d.line + 1;
    let display_col = d.col + 1;

    // Apply severity color to a string slice when color is enabled.
    let sev_color = |s: &str| -> String {
        if !color {
            return s.to_owned();
        }
        match d.severity {
            DiagnosticSeverity::Error   => s.red().bold().to_string(),
            DiagnosticSeverity::Warning => s.yellow().bold().to_string(),
            DiagnosticSeverity::Info    => s.cyan().bold().to_string(),
            DiagnosticSeverity::Hint    => s.dimmed().to_string(),
        }
    };
    let blue = |s: &str| -> String {
        if color { s.blue().to_string() } else { s.to_owned() }
    };
    let msg_styled = if color { d.message.bold().to_string() } else { d.message.clone() };

    let mut out = String::new();
    out.push_str(&format!("{}: {}\n", sev_color(&format!("{} {}", d.code, d.slug)), msg_styled));
    out.push_str(&format!(" --> {}:{}:{}\n", d.file, display_line, display_col));

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
fn format_rich_diagnostic_for_source(
    d: &jinja_lsp::diagnostic::Diagnostic,
    src_line: &str,
) -> String {
    format_rich_diagnostic_colored(d, src_line, false)
}

fn collect_template_files(dir: &std::path::Path, exts: &[&str], out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_template_files(&path, exts, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if exts.contains(&ext) {
                out.push(path);
            }
        }
    }
}

#[cfg(test)]
fn capture_unified_diff(path: &std::path::Path, original: &str, formatted: &str) -> String {
    use similar::{ChangeTag, TextDiff};
    let diff = TextDiff::from_lines(original, formatted);
    let display = path.display();
    let mut out = format!("--- {display}\n+++ {display} (formatted)\n");
    for group in diff.grouped_ops(3) {
        let first = group.first().unwrap();
        let last = group.last().unwrap();
        let old_range = first.old_range().start..last.old_range().end;
        let new_range = first.new_range().start..last.new_range().end;
        out.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            old_range.start + 1, old_range.len(),
            new_range.start + 1, new_range.len(),
        ));
        for op in &group {
            for change in diff.iter_changes(op) {
                let prefix = match change.tag() {
                    ChangeTag::Delete => '-',
                    ChangeTag::Insert => '+',
                    ChangeTag::Equal  => ' ',
                };
                out.push(prefix);
                out.push_str(&change.to_string());
                if change.missing_newline() { out.push('\n'); }
            }
        }
    }
    out
}

fn print_unified_diff(path: &std::path::Path, original: &str, formatted: &str) {
    use similar::{ChangeTag, TextDiff};

    let diff = TextDiff::from_lines(original, formatted);
    let display = path.display();
    println!("--- {display}");
    println!("+++ {display} (formatted)");
    for group in diff.grouped_ops(3) {
        let first = group.first().unwrap();
        let last = group.last().unwrap();
        let old_range = first.old_range().start..last.old_range().end;
        let new_range = first.new_range().start..last.new_range().end;
        println!(
            "@@ -{},{} +{},{} @@",
            old_range.start + 1, old_range.len(),
            new_range.start + 1, new_range.len(),
        );
        for op in &group {
            for change in diff.iter_changes(op) {
                let prefix = match change.tag() {
                    ChangeTag::Delete => '-',
                    ChangeTag::Insert => '+',
                    ChangeTag::Equal  => ' ',
                };
                print!("{prefix}{change}");
                if !change.missing_newline() { } else { println!(); }
            }
        }
    }
}
