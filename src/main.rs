use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "jinja-lsp", about = "Jinja2 template language server")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the LSP server (stdio transport)
    Lsp,
    /// Check templates for diagnostics
    Check {
        /// Files or directories to check
        paths: Vec<String>,
    },
    /// Format Jinja templates
    Format {
        /// Files to format
        paths: Vec<String>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Lsp) {
        Commands::Lsp => run_lsp().await,
        Commands::Check { paths: _ } => run_check(),
        Commands::Format { paths: _ } => run_format(),
    }
}

async fn run_lsp() {
    todo!("LSP server not yet implemented")
}

fn run_check() {
    todo!("check not yet implemented")
}

fn run_format() {
    todo!("format not yet implemented")
}
