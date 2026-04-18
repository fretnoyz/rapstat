mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rapstat", version, about = "Project observability tool")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Install git hooks into .git/hooks/ for the current repo
    Init,
    /// Scan current project and update STATUS.md
    Scan,
    /// Print current STATUS.md to stdout
    Status,
    /// Validate CONTEXT.md against git log and print discrepancies (read-only)
    Check,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init => commands::init::run(),
        Command::Scan => commands::scan::run(),
        Command::Status => commands::status::run(),
        Command::Check => commands::check::run(),
    }
}
