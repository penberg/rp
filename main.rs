mod agent;
mod cmd;
mod config;
mod issues;

use crate::cmd::config::ConfigCommand;
use clap::{Parser, Subcommand};
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "rp")]
#[command(about = "rp CLI", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize rp manifest
    Init,
    /// Run the current issue reproducer
    Check {
        issue: Option<String>,
    },
    /// Fix the current issue in the tree
    Fix,
    /// Inspect a bug report and materialize a local reproducer workspace
    Inspect {
        #[arg(long)]
        verbose: bool,
        prompt: String,
    },
    /// Manage repository options
    Config {
        #[command(subcommand)]
        command: Option<ConfigCommand>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => cmd::init::run(),
        Commands::Check { issue } => cmd::check::run(issue.as_deref()),
        Commands::Fix => cmd::fix::run(),
        Commands::Inspect { verbose, prompt } => cmd::inspect::run(&prompt, verbose),
        Commands::Config { command } => cmd::config::run(command),
    }
}
