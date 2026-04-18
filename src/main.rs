mod commands;
mod config;
mod context_check;
mod git;
mod status_model;

use anyhow::Result;
use clap::{Parser, Subcommand};
use commands::scan::TriggerArg;

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
    Scan {
        /// Trigger context: commit (pre-commit hook), push (pre-push hook), or scan (manual/cron)
        #[arg(long, default_value = "scan")]
        trigger: TriggerArg,
    },
    /// Print current STATUS.md to stdout
    Status,
    /// Validate CONTEXT.md against git log and print discrepancies (read-only)
    Check,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init => commands::init::run(),
        Command::Scan { trigger } => commands::scan::run(trigger),
        Command::Status => commands::status::run(),
        Command::Check => commands::check::run(),
    }
}
