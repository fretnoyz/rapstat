mod commands;
mod config;
mod context_check;
mod git;
mod registry;
mod status_model;

use anyhow::Result;
use clap::{Parser, Subcommand};
use commands::scan::TriggerArg;
use std::path::PathBuf;

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
    /// Manage the global project registry (~/.rapstat/projects.toml)
    Projects {
        #[command(subcommand)]
        action: ProjectsAction,
    },
}

#[derive(Subcommand)]
enum ProjectsAction {
    /// List all registered projects
    List,
    /// Register a project (defaults to current directory)
    Add {
        /// Path to the project root (defaults to current directory)
        path: Option<PathBuf>,
        /// Override the project name (inferred from .rapstat/config.toml if absent)
        #[arg(long)]
        name: Option<String>,
    },
    /// Deregister a project by name
    Remove {
        /// The project name to remove
        name: String,
    },
    /// Run a scan on every registered project
    Scan,
    /// Import projects from a raptor.yaml file
    Import {
        /// Path to raptor.yaml (e.g. ~/repos/raptor-toolkit/raptor.yaml)
        raptor_yaml: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init => commands::init::run(),
        Command::Scan { trigger } => commands::scan::run(trigger),
        Command::Status => commands::status::run(),
        Command::Check => commands::check::run(),
        Command::Projects { action } => match action {
            ProjectsAction::List => commands::projects::list(),
            ProjectsAction::Add { path, name } => commands::projects::add(path, name),
            ProjectsAction::Remove { name } => commands::projects::remove(&name),
            ProjectsAction::Scan => commands::projects::scan_all(),
            ProjectsAction::Import { raptor_yaml } => commands::projects::import(raptor_yaml),
        },
    }
}
