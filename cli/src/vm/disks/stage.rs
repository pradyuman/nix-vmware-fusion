use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

use crate::config::CONFIG;
use crate::vm::schema::DiskBus;

use super::{Action, DiskLabel, Plan};

#[derive(Deserialize)]
struct FreeDiskLabel {
    #[serde(rename = "FindFirstFree")]
    label: DiskLabel,
}

pub(crate) fn stage(draft_path: &Path, plan: Plan) -> Result<()> {
    plan.actions.into_iter().try_for_each(|action| {
        match action {
            Action::Move { from, to } => {
                let target = find_first_free(draft_path, to)?;

                duct::cmd!(&CONFIG.vmcli, draft_path, "disk", "move", &from, &target)
                    .run()
                    .with_context(|| format!("could not move disk {from} to {target}"))?;
            }
            Action::Attach { path, to } => {
                let label = find_first_free(draft_path, to)?;

                // Configure the backing file before making the device present
                duct::cmd!(
                    &CONFIG.vmcli,
                    draft_path,
                    "disk",
                    "setbackinginfo",
                    &label,
                    "disk",
                    &path,
                    "false"
                )
                .run()
                .with_context(|| format!("could not attach disk {}", path.display()))?;

                duct::cmd!(
                    &CONFIG.vmcli,
                    draft_path,
                    "disk",
                    "setpresent",
                    &label,
                    "true"
                )
                .run()
                .with_context(|| format!("could not attach disk {}", path.display()))?;
            }
            Action::Detach { label } => {
                duct::cmd!(&CONFIG.vmcli, draft_path, "disk", "purge", &label)
                    .run()
                    .with_context(|| format!("could not detach disk {label}"))?;
            }
        }

        Ok(())
    })
}

fn find_first_free(draft_path: &Path, bus: DiskBus) -> Result<DiskLabel> {
    let (module, controller) = match bus {
        DiskBus::Nvme => ("nvme", "nvme0"),
        DiskBus::Sata => ("sata", "sata0"),
    };

    duct::cmd!(
        &CONFIG.vmcli,
        draft_path,
        module,
        "setpresent",
        controller,
        "true"
    )
    .run()
    .with_context(|| format!("could not enable {controller}"))?;

    // Query the evolving draft so each action sees labels freed or claimed earlier
    let json = duct::cmd!(
        &CONFIG.vmcli,
        draft_path,
        module,
        "findfirstfree",
        controller,
        "-f",
        "json"
    )
    .read()
    .context("could not find a free disk label")?;

    Ok(serde_json::from_str::<FreeDiskLabel>(&json)
        .context("could not parse vmcli disk label JSON")?
        .label)
}
