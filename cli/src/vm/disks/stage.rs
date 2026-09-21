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
                Action::Convert { path, format } => {
                    // Defer changes to the real disk until commit checks the VM is stopped
                    Some(CommitAction::Convert { path, format })
                }
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
    fn expansion_is_staged_for_commit() -> Result<()> {
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

    #[test]
    fn conversion_is_staged_for_commit() -> Result<()> {
        let disk_path = Path::new("/disks/system.vmdk");
        let format = DiskFormat::SplitSparse;
        let plan = Plan {
            actions: vec![Action::Convert {
                path: disk_path.to_owned(),
                format,
            }],
        };

        let staged = stage(Path::new("unused.vmx"), plan)?;

        assert!(matches!(
            staged.commit_actions.as_slice(),
            [CommitAction::Convert {
                path,
                format: staged_format,
            }] if path == disk_path && *staged_format == format
        ));

        Ok(())
    }

    #[cfg(feature = "vmware-tests")]
    mod vmware {
        use std::fs;
        use std::path::Path;

        use crate::vm::disks::{BYTES_PER_GIB, inspect};
        use crate::vm::schema::{DiskPath, VirtualDisk, VirtualDisks};
        use crate::vm::test_support::{create_partitioned_vmdk, create_vmx};

        use super::*;

        const BYTES_PER_MIB: u64 = 1024_u64.pow(2);

        fn create_vmdk(path: &Path, size: &str) -> Result<()> {
            duct::cmd!(
                &CONFIG.vdisk_manager,
                "-c",
                "-s",
                size,
                "-a",
                "lsilogic",
                "-t",
                "0",
                "-q",
                path
            )
            .run()?;

            Ok(())
        }

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
        fn vmware_tools_report_and_expand_disk_capacity() -> Result<()> {
            let temp_dir = tempfile::tempdir()?;
            let vmx_path = temp_dir.path().join("missing.vmx");
            let disk_path = temp_dir.path().join("managed.vmdk");
            let expanded_size = NonZeroU64::new(1).expect("non-zero disk capacity");

            create_partitioned_vmdk(&disk_path, "10MiB")?;

            let configured = VirtualDisks::from([(
                "primary".to_owned(),
                VirtualDisk {
                    path: DiskPath::try_new(disk_path.clone()).expect("valid disk path"),
                    size: expanded_size,
                    bus: DiskBus::Nvme,
                    preallocate: false,
                    split: false,
                },
            )]);

            // Inspect the initial capacity
            let snapshot = inspect(&vmx_path, &configured)?;
            assert_eq!(
                snapshot.configured_disks[0]
                    .current_state
                    .capacity_bytes
                    .get(),
                10 * BYTES_PER_MIB
            );

            // Expand the disk
            stage(
                &vmx_path,
                Plan {
                    actions: vec![Action::Expand {
                        path: disk_path,
                        size: expanded_size,
                    }],
                },
            )?
            .commit()?;

            // Inspect the expanded capacity
            let snapshot = inspect(&vmx_path, &configured)?;
            assert_eq!(
                snapshot.configured_disks[0]
                    .current_state
                    .capacity_bytes
                    .get(),
                expanded_size.get() * BYTES_PER_GIB
            );

            Ok(())
        }
    }
}
