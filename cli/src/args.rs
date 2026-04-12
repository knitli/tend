//! T054: Clap-derive argument parsing for the `agentui` CLI wrapper.

use clap::Parser;

/// Agent UI CLI wrapper — run commands with workbench session tracking.
#[derive(Parser, Debug)]
#[command(name = "agentui", version, about)]
pub enum Cli {
    /// Run a command with agent UI session tracking.
    Run(RunArgs),
}

/// Arguments for `agentui run`.
#[derive(clap::Args, Debug)]
pub struct RunArgs {
    /// Project name or path (defaults to $PWD).
    #[arg(short, long)]
    pub project: Option<String>,

    /// Human-readable session label.
    #[arg(short, long)]
    pub label: Option<String>,

    /// Working directory override (defaults to project path).
    #[arg(short = 'd', long)]
    pub working_directory: Option<String>,

    /// Command to run (everything after `--`).
    #[arg(last = true, required = true)]
    pub command: Vec<String>,
}
