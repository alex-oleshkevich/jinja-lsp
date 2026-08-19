// REQ-FOLD-01: main.rs only parses the CLI and routes. No analysis logic here —
// `check` goes to linter/, `format` to format/cli.rs, `lsp` to server/.
use clap::{Parser, Subcommand};
use jinja_lsp::doctor::run_doctor;
use jinja_lsp::format::cli::run_format;
use jinja_lsp::linter::run_check;

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
        #[arg(long, short = 'c')]
        config: Option<String>,
        /// Enable only these diagnostic codes/prefixes
        #[arg(long, value_delimiter = ',')]
        select: Vec<String>,
        /// Disable these diagnostic codes/prefixes
        #[arg(long, value_delimiter = ',')]
        ignore: Vec<String>,
    },
    /// Report what jinja-lsp discovers here: config, templates, and builtins
    Doctor {
        /// Path to config file (overrides discovery)
        #[arg(long, short = 'c')]
        config: Option<String>,
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
        /// Write formatted output to PATH instead of editing in place; use '-' for stdout
        #[arg(long, value_name = "PATH")]
        output: Option<String>,
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
        Commands::Check {
            paths,
            format,
            verbose,
            config,
            select,
            ignore,
        } => run_check(paths, &format, verbose, config.as_deref(), &select, &ignore),
        Commands::Doctor { config } => run_doctor(config.as_deref()),
        Commands::Format {
            paths,
            config,
            check,
            diff,
            output,
        } => run_format(paths, config.as_deref(), check, diff, output.as_deref()),
    };
    std::process::exit(code);
}
