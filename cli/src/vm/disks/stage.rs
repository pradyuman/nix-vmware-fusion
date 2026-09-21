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

    #[cfg(feature = "vmware-contract-tests")]
    mod vmware {
        use std::fs;
        use std::path::Path;

        use crate::vm::disks::{BYTES_PER_GIB, inspect};
        use crate::vm::schema::{VirtualDisk, VirtualDiskPath, VirtualDisks};
        use crate::vm::test_support::create_vmx;

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

        fn create_partitioned_vmdk(directory: &Path, path: &Path, size: &str) -> Result<()> {
            let source_path = directory.join("source.raw");

            // Create a partition table so vmdkserver can report the disk capacity
            duct::cmd!(
                "diskutil",
                "image",
                "create",
                "blank",
                "--format",
                "RAW",
                "--size",
                size,
                "--fs",
                "ExFAT",
                &source_path
            )
            .run()?;

            duct::cmd!(
                "qemu-img",
                "convert",
                "-f",
                "raw",
                "-O",
                "vmdk",
                &source_path,
                path,
            )
            .run()?;

            Ok(())
        }

        fn configured_disks(path: &Path, size: NonZeroU64) -> VirtualDisks {
            VirtualDisks::from([(
                "primary".to_owned(),
                VirtualDisk {
                    path: VirtualDiskPath::try_new(path.to_owned()).expect("valid disk path"),
                    size,
                    bus: VirtualDiskBus::Nvme,
                },
            )])
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
                        path: disk_path.clone(),
                        to: VirtualDiskBus::Nvme,
                    }],
                },
            )?;

            let attached = inspect(&draft_path, &VirtualDisks::new())?.attached_disks;
            assert_eq!(attached.len(), 1);
            assert!(attached[0].label.starts_with("nvme"));
            assert_eq!(
                attached[0].canonical_path.as_deref(),
                Some(canonical_path.as_path())
            );

            // Move the attached disk to SATA
            let nvme_label = attached[0].label.clone();
            stage(
                &draft_path,
                Plan {
                    actions: vec![Action::Move {
                        from: nvme_label,
                        to: VirtualDiskBus::Sata,
                    }],
                },
            )?;

            let attached = inspect(&draft_path, &VirtualDisks::new())?.attached_disks;
            assert_eq!(attached.len(), 1);
            assert!(attached[0].label.starts_with("sata"));
            assert_eq!(
                attached[0].canonical_path.as_deref(),
                Some(canonical_path.as_path())
            );

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

            create_partitioned_vmdk(temp_dir.path(), &disk_path, "10MiB")?;

            let configured = configured_disks(&disk_path, expanded_size);

            // Inspect the initial capacity
            let snapshot = inspect(&vmx_path, &configured)?;
            assert_eq!(
                snapshot.configured_disks[0].current_bytes.get(),
                10 * BYTES_PER_MIB
            );

            // Expand the disk
            stage(
                &vmx_path,
                Plan {
                    actions: vec![Action::Expand {
                        path: disk_path.clone(),
                        size: expanded_size,
                    }],
                },
            )?
            .commit()?;

            // Inspect the expanded capacity
            let snapshot = inspect(&vmx_path, &configured)?;
            assert_eq!(
                snapshot.configured_disks[0].current_bytes.get(),
                expanded_size.get() * BYTES_PER_GIB
            );

            Ok(())
        }
    }
}
