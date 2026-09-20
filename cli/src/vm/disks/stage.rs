use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

use crate::config::CONFIG;
use crate::vm::schema::VirtualDiskBus;

use super::{Action, CommitAction, DiskLabel, Plan, StagedChange};

#[derive(Deserialize)]
struct FreeDiskLabel {
    #[serde(rename = "FindFirstFree")]
    label: DiskLabel,
}

pub(crate) fn stage(draft_path: &Path, plan: Plan) -> Result<StagedChange> {
    let commit_actions = plan
        .actions
        .into_iter()
        .map(|action| -> Result<Option<CommitAction>> {
            let commit_action = match action {
                Action::Move { from, to } => {
                    let target = find_first_free(draft_path, to)?;

                    duct::cmd!(&CONFIG.vmcli, draft_path, "disk", "move", &from, &target)
                        .run()
                        .with_context(|| format!("could not move disk {from} to {target}"))?;

                    None
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

                    None
                }
                Action::Detach { label } => {
                    duct::cmd!(&CONFIG.vmcli, draft_path, "disk", "purge", &label)
                        .run()
                        .with_context(|| format!("could not detach disk {label}"))?;

                    None
                }
                Action::Expand { path, size } => {
                    // Defer changes to the real disk until commit checks the VM is stopped
                    Some(CommitAction::Expand { path, size })
                }
            };

            Ok(commit_action)
        })
        .filter_map(Result::transpose)
        .collect::<Result<Vec<_>>>()?;

    Ok(StagedChange { commit_actions })
}

fn find_first_free(draft_path: &Path, bus: VirtualDiskBus) -> Result<DiskLabel> {
    let (module, controller) = match bus {
        VirtualDiskBus::Nvme => ("nvme", "nvme0"),
        VirtualDiskBus::Sata => ("sata", "sata0"),
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

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use super::*;

    #[test]
    fn expansion_is_deferred_until_commit() -> Result<()> {
        let disk_path = Path::new("/disks/system.vmdk");
        let size = NonZeroU64::new(12).expect("non-zero disk capacity");
        let plan = Plan {
            actions: vec![Action::Expand {
                path: disk_path.to_owned(),
                size,
            }],
        };

        let staged = stage(Path::new("unused.vmx"), plan)?;

        assert!(matches!(
            staged.commit_actions.as_slice(),
            [CommitAction::Expand {
                path,
                size: staged_size,
            }] if path == disk_path && *staged_size == size
        ));

        Ok(())
    }
}
