use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand};

mod config;
mod network;
mod script;
mod state_file;
mod vm;

#[derive(Parser)]
#[command(version, about = "Manage VMware Fusion")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage system-wide networks
    Network {
        #[command(subcommand)]
        command: NetworkCommands,
    },

    /// Create or update virtual machines
    Vm {
        #[command(subcommand)]
        command: VmCommands,
    },

    /// Install VMware Fusion from a DMG
    Install { dmg: PathBuf },

    /// Uninstall VMware Fusion
    Uninstall,

    /// Uninstall VMware Fusion and remove its support files
    Purge {
        /// Skip confirmation
        #[arg(long)]
        yes: bool,

        /// Select the user whose files will be removed
        #[arg(long)]
        user: Option<String>,
    },
}

#[derive(Subcommand)]
enum NetworkCommands {
    /// Apply a network configuration from a JSON file
    Apply { file: PathBuf },
}

#[derive(Subcommand)]
enum VmCommands {
    /// Apply virtual machine configurations from JSON files
    Apply {
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
}

fn main() -> Result<ExitCode> {
    match Cli::parse().command {
        Commands::Network {
            command: NetworkCommands::Apply { file },
        } => {
            network::apply(&file)?;
            Ok(ExitCode::SUCCESS)
        }
        Commands::Vm {
            command: VmCommands::Apply { files },
        } => {
            vm::apply_all(&files)?;
            Ok(ExitCode::SUCCESS)
        }
        Commands::Install { dmg } => script::run(
            "install.sh",
            include_str!("../scripts/install.sh"),
            &[dmg.into_os_string()],
        ),
        Commands::Uninstall => {
            script::run("uninstall.sh", include_str!("../scripts/uninstall.sh"), &[])
        }
        Commands::Purge { yes, user } => script::run(
            "purge.sh",
            include_str!("../scripts/purge.sh"),
            &[yes.to_string().into(), user.unwrap_or_default().into()],
        ),
    }
}
