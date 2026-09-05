use anyhow::Result;
use clap::{Parser, Subcommand};
use std::env;
use std::process::{Command, ExitCode};

#[derive(Parser)]
#[command(version, about = "Manage VMware Fusion")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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

fn shell(name: &str, contents: &str) -> Command {
    let mut command = Command::new("bash");
    command.args(["-c", contents, name]);
    command
}

fn main() -> Result<ExitCode> {
    let mut command = match Cli::parse().command {
        Commands::Install => {
            let dmg = env::var_os("VMWARE_FUSION_DMG").ok_or(env::VarError::NotPresent)?;
            let mut command = shell("install.sh", include_str!("../scripts/install.sh"));
            command.arg(dmg);
            command
        }
        Commands::Uninstall => shell("uninstall.sh", include_str!("../scripts/uninstall.sh")),
        Commands::Purge { yes, user } => {
            let mut command = shell("purge.sh", include_str!("../scripts/purge.sh"));
            command.arg(yes.to_string());
            command.arg(user.unwrap_or_default());
            command
        }
    };

    let status = command.status()?;
    Ok(status
        .code()
        .map_or(ExitCode::FAILURE, |code| ExitCode::from(code as u8)))
}
