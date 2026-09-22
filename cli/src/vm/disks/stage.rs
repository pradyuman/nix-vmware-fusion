use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

use crate::config::CONFIG;
use crate::vm::schema::DiskBus;

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
                Action::Create { path, size, format } => {
                    Some(CommitAction::Create { path, size, format })
                }
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
                Action::Expand { path, size } => Some(CommitAction::Expand { path, size }),
                Action::Convert { path, format } => Some(CommitAction::Convert { path, format }),
            };

            Ok(commit_action)
        })
        .filter_map(Result::transpose)
        .collect::<Result<Vec<_>>>()?;

    Ok(StagedChange { commit_actions })
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

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use crate::vm::disks::DiskFormat;

    use super::*;

    #[test]
    fn disk_changes_are_staged_for_commit() -> Result<()> {
        let disk_path = Path::new("/disks/system.vmdk");
        let size = NonZeroU64::new(12).expect("non-zero disk capacity");
        let format = DiskFormat::SplitSparse;

        let plan = Plan {
            actions: vec![
                Action::Create {
                    path: disk_path.to_owned(),
                    size,
                    format,
                },
                Action::Expand {
                    path: disk_path.to_owned(),
                    size,
                },
                Action::Convert {
                    path: disk_path.to_owned(),
                    format,
                },
            ],
        };

        let staged = stage(Path::new("unused.vmx"), plan)?;

        assert_eq!(
            staged.commit_actions,
            vec![
                CommitAction::Create {
                    path: disk_path.to_owned(),
                    size,
                    format,
                },
                CommitAction::Expand {
                    path: disk_path.to_owned(),
                    size,
                },
                CommitAction::Convert {
                    path: disk_path.to_owned(),
                    format,
                },
            ]
        );

        Ok(())
    }

    #[cfg(feature = "vmware-tests")]
    mod vmware {
        use std::fs;

        use crate::vm::disks::inspect;
        use crate::vm::schema::VirtualDisks;
        use crate::vm::test_support::{create_vmdk, create_vmx};

        use super::*;

        #[test]
        fn vmcli_manages_disk_attachments() -> Result<()> {
            let (temp_dir, draft_path) = create_vmx()?;
            let disk_path = temp_dir.path().join("managed.vmdk");

            create_vmdk(&disk_path, "1MB")?;
            let canonical_path = fs::canonicalize(&disk_path)?;

            // Attach the disk over NVMe
            stage(
                &draft_path,
                Plan {
                    actions: vec![Action::Attach {
                        path: disk_path,
                        to: DiskBus::Nvme,
                    }],
                },
            )?;

            let attached = inspect(&draft_path, &VirtualDisks::new())?.attached_disks;
            assert_eq!(attached.len(), 1);
            assert!(attached[0].label.starts_with("nvme"));
            assert_eq!(attached[0].canonical_path.as_ref(), Some(&canonical_path));

            // Move the attached disk to SATA
            let nvme_label = attached[0].label.clone();
            stage(
                &draft_path,
                Plan {
                    actions: vec![Action::Move {
                        from: nvme_label,
                        to: DiskBus::Sata,
                    }],
                },
            )?;

            let attached = inspect(&draft_path, &VirtualDisks::new())?.attached_disks;
            assert_eq!(attached.len(), 1);
            assert!(attached[0].label.starts_with("sata"));
            assert_eq!(attached[0].canonical_path, Some(canonical_path));

            // Detach the disk
            stage(
                &draft_path,
                Plan {
                    actions: vec![Action::Detach {
                        label: attached[0].label.clone(),
                    }],
                },
            )?;

            assert!(
                inspect(&draft_path, &VirtualDisks::new())?
                    .attached_disks
                    .is_empty()
            );

            Ok(())
        }

        #[test]
        fn vmcli_configures_missing_disk_backing_path() -> Result<()> {
            let (temp_dir, draft_path) = create_vmx()?;
            let disk_path = temp_dir.path().join("missing/test.vmdk");

            stage(
                &draft_path,
                Plan {
                    actions: vec![Action::Attach {
                        path: disk_path,
                        to: DiskBus::Nvme,
                    }],
                },
            )?;

            let attached = inspect(&draft_path, &VirtualDisks::new())?.attached_disks;
            assert_eq!(attached.len(), 1);
            assert!(attached[0].label.starts_with("nvme"));
            assert!(attached[0].canonical_path.is_none());

            Ok(())
        }
    }
}
