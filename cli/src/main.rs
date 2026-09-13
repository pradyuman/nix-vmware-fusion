use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

use crate::config::CONFIG;

mod config;
mod script;
mod vm;

#[derive(Parser)]
#[command(version, about = "Manage VMware Fusion")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create or update virtual machines
    Vm {
        #[command(subcommand)]
        command: VmCommands,
    },

    /// Install VMware Fusion
    Install,

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
enum VmCommands {
    /// Apply a virtual machine configuration from a JSON file
    Apply { file: PathBuf },
}

fn main() -> Result<ExitCode> {
    match Cli::parse().command {
        Commands::Vm {
            command: VmCommands::Apply { file },
        } => {
            vm::apply(&file)?;
            Ok(ExitCode::SUCCESS)
        }
        Commands::Install => script::run(
            "install.sh",
            include_str!("../scripts/install.sh"),
            &[CONFIG.dmg.as_os_str().to_owned()],
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
