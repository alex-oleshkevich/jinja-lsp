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
    /// Check templates for diagnostics
    Check {
        /// Files or directories to check
        paths: Vec<String>,
        /// Output format: "text" (default) or "json"
        #[arg(long, default_value = "text")]
        format: String,
    },
    /// Format Jinja templates
    Format {
        paths: Vec<String>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Commands::Lsp) {
        Commands::Lsp => jinja_lsp::server::run_lsp_server().await,
        Commands::Check { paths, format } => run_check(paths, &format),
        Commands::Format { paths: _ } => run_format(),
    }
}

/// REQ-E2E-03: `check --format json` outputs a JSON array of Diagnostic objects
/// matching the canonical shape from E17.
fn run_check(paths: Vec<String>, format: &str) {
    use std::path::Path;
    use jinja_lsp::workspace::build_workspace;
    use jinja_lsp::diagnostic::Diagnostic;

    // Collect all template dirs from the provided paths
    let dirs: Vec<std::path::PathBuf> = paths.iter().map(|p| Path::new(p).to_owned()).collect();
    let dir_refs: Vec<&Path> = dirs.iter().map(|d| d.as_path()).collect();

    // Build the workspace (Pass 1 + relink)
    let _workspace = build_workspace(&dir_refs, &["html", "jinja", "jinja2", "j2"]);

    // Collect diagnostics — F01 not yet implemented; emit empty list
    let diagnostics: Vec<Diagnostic> = vec![];

    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&diagnostics)
                .expect("serialization must not fail");
            println!("{json}");
        }
        _ => {
            for d in &diagnostics {
                eprintln!("{}:{}:{}: {} [{}]", d.file, d.line, d.col, d.message, d.code);
            }
        }
    }
}

fn run_format() {
    todo!("format not yet implemented")
}
