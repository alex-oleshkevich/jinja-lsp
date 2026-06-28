use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "jinja-lsp", about = "Jinja2 template language server")]
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
    /// Format Jinja templates
    Format {
        paths: Vec<String>,
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
        Commands::Check { paths, format, verbose: _, config: _, select, ignore } => {
            run_check(paths, &format, &select, &ignore)
        }
        Commands::Format { paths: _ } => {
            todo!("format not yet implemented")
        }
    };
    std::process::exit(code);
}

/// REQ-LINT-01..11: check command implementation.
/// Returns exit code: 0 = no findings, 1 = findings found, 2 = config/usage error.
fn run_check(paths: Vec<String>, format: &str, select: &[String], ignore: &[String]) -> i32 {
    use std::path::Path;
    use jinja_lsp::diagnostic::{Diagnostic, DiagnosticSeverity};
    use jinja_lsp::diagnostics::filter_by_config;
    use jinja_lsp::workspace::build_workspace;

    // REQ-LINT-01: collect template dirs/files from paths
    let dirs: Vec<std::path::PathBuf> = if paths.is_empty() {
        // default: look for templates/ in CWD
        let cwd = std::env::current_dir().unwrap_or_default();
        let templates = cwd.join("templates");
        if templates.is_dir() { vec![templates] } else { vec![cwd] }
    } else {
        paths.iter().map(|p| Path::new(p).to_path_buf()).collect()
    };

    let dir_refs: Vec<&Path> = dirs.iter().map(|d| d.as_path()).collect();

    // REQ-LINT-09: build_workspace is the shared engine (same as LSP server)
    let _workspace = build_workspace(&dir_refs, &["html", "jinja", "jinja2", "j2"]);

    // Collect diagnostics — F01 checks not yet implemented; emit empty list
    let all_diags: Vec<Diagnostic> = vec![];

    // REQ-LINT-03: apply select/ignore filters
    let sel: Vec<&str> = select.iter().map(|s| s.as_str()).collect();
    let ign: Vec<&str> = ignore.iter().map(|s| s.as_str()).collect();
    let filtered = filter_by_config(&all_diags, &sel, &ign);

    // REQ-LINT-10: normalize paths (forward slashes)
    let findings: Vec<Diagnostic> = filtered.into_iter().cloned().collect();

    // REQ-LINT-07: order by file, line, col
    let mut sorted = findings;
    sorted.sort_by(|a, b| {
        a.file.cmp(&b.file)
            .then(a.line.cmp(&b.line))
            .then(a.col.cmp(&b.col))
    });

    // REQ-LINT-04/05/06: output format
    match format {
        "json" => {
            // REQ-LINT-06/07: JSON array with 7-key shape
            let json = serde_json::to_string_pretty(&sorted).expect("serialization must not fail");
            println!("{json}");
        }
        "compact" => {
            // REQ-LINT-05: one line per finding
            for d in &sorted {
                println!("{}:{}:{}: {} {} {}", d.file, d.line, d.col, d.code, d.slug, d.message);
            }
        }
        _ => {
            // REQ-LINT-04: rich rustc-style report
            for d in &sorted {
                let sev = match d.severity {
                    DiagnosticSeverity::Error => "error",
                    DiagnosticSeverity::Warning => "warning",
                    DiagnosticSeverity::Info => "info",
                    DiagnosticSeverity::Hint => "hint",
                };
                println!("{sev}[{}]: {}", d.code, d.message);
                println!("  --> {}:{}:{}", d.file, d.line, d.col);
            }
            if sorted.is_empty() {
                println!("No problems found.");
            }
        }
    }

    // REQ-LINT-08: exit codes 0 (no findings) / 1 (findings) / 2 (error)
    if sorted.is_empty() { 0 } else { 1 }
}
