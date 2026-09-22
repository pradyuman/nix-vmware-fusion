use anyhow::{Context, Result, bail, ensure};
use std::cmp::Ordering;
use std::collections::HashSet;

use crate::vm::schema::DiskBus;

use super::{Action, BYTES_PER_GIB, InspectedDisk, Plan, Snapshot};

pub(crate) fn plan(snapshot: Snapshot) -> Result<Plan> {
    let identity_paths = snapshot
        .inspected_disks
        .iter()
        .map(|disk| &disk.identity_path)
        .collect::<HashSet<_>>();

    ensure!(
        identity_paths.len() == snapshot.inspected_disks.len(),
        "disk paths must be unique"
    );

    let attachments = snapshot.inspected_disks.iter().filter_map(|disk| {
        let configured = &disk.configured;
        let attached = snapshot
            .attached_disks
            .iter()
            .find(|attached| attached.canonical_path.as_ref() == Some(&disk.identity_path));

        match attached {
            Some(attached) => {
                let bus = bus_from_label(&attached.label);

                (bus != Some(configured.bus)).then(|| Action::Move {
                    from: attached.label.clone(),
                    to: configured.bus,
                })
            }
            None => Some(Action::Attach {
                path: configured.path.clone(),
                to: configured.bus,
            }),
        }
    });

    let detachments = snapshot
        .attached_disks
        .iter()
        .filter(|attached| {
            !snapshot
                .inspected_disks
                .iter()
                .any(|disk| attached.canonical_path.as_ref() == Some(&disk.identity_path))
        })
        .map(|attached| Action::Detach {
            label: attached.label.clone(),
        });

    let creations = snapshot
        .inspected_disks
        .iter()
        .filter(|disk| disk.current_state.is_none())
        .map(|disk| Action::Create {
            path: disk.configured.path.clone(),
            size: disk.configured.size,
            format: disk.configured.format,
        });

    let expansions = snapshot
        .inspected_disks
        .iter()
        .map(plan_expansion)
        .filter_map(Result::transpose)
        .collect::<Result<Vec<_>>>()?;

    let conversions = snapshot.inspected_disks.iter().filter_map(|disk| {
        disk.current_state
            .as_ref()
            .filter(|state| disk.configured.format != state.format)
            .map(|_| Action::Convert {
                path: disk.configured.path.clone(),
                format: disk.configured.format,
            })
    });

    // Detach unconfigured disks first so moves and attachments can reuse their labels
    Ok(Plan {
        actions: detachments
            .chain(creations)
            .chain(attachments)
            .chain(expansions)
            .chain(conversions)
            .collect(),
    })
}

fn plan_expansion(disk: &InspectedDisk) -> Result<Option<Action>> {
    let Some(current_state) = &disk.current_state else {
        return Ok(None);
    };

    let requested_bytes = disk
        .configured
        .size
        .get()
        .checked_mul(BYTES_PER_GIB)
        .context("configured disk capacity is too large")?;

    match requested_bytes.cmp(&current_state.capacity_bytes.get()) {
        Ordering::Less => bail!(
            "cannot resize disk {} below its current capacity (requested: {} GiB)",
            disk.configured.path.display(),
            disk.configured.size
        ),
        Ordering::Greater => Ok(Some(Action::Expand {
            path: disk.configured.path.clone(),
            size: disk.configured.size,
        })),
        Ordering::Equal => Ok(None),
    }
}

fn bus_from_label(label: &str) -> Option<DiskBus> {
    if label.starts_with("nvme") {
        Some(DiskBus::Nvme)
    } else if label.starts_with("sata") {
        Some(DiskBus::Sata)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;
    use std::path::Path;

    use crate::vm::disks::{AttachedDisk, ConfiguredDisk, DiskFormat, DiskState, InspectedDisk};

    use super::*;

    fn inspected_disk(path: &Path, bus: DiskBus) -> InspectedDisk {
        InspectedDisk {
            configured: ConfiguredDisk {
                path: path.to_owned(),
                size: NonZeroU64::new(8).expect("non-zero disk capacity"),
                bus,
                format: DiskFormat::Sparse,
            },
            identity_path: path.to_owned(),
            current_state: Some(DiskState {
                capacity_bytes: NonZeroU64::new(8 * BYTES_PER_GIB)
                    .expect("non-zero current disk capacity"),
                format: DiskFormat::Sparse,
            }),
        }
    }

    fn inspected_disk_with_capacity(path: &Path, size: u64, current_bytes: u64) -> InspectedDisk {
        let mut disk = inspected_disk(path, DiskBus::Nvme);
        disk.configured.size = NonZeroU64::new(size).expect("non-zero disk capacity");
        disk.current_state = Some(DiskState {
            capacity_bytes: NonZeroU64::new(current_bytes).expect("non-zero current disk capacity"),
            format: DiskFormat::Sparse,
        });

        disk
    }

    fn attached_disk(path: &Path, label: &str) -> AttachedDisk {
        AttachedDisk {
            label: label.to_owned(),
            canonical_path: Some(path.to_owned()),
        }
    }

    #[test]
    fn disk_on_configured_bus_is_unchanged() -> Result<()> {
        let disk_path = Path::new("/disks/system.vmdk");
        let snapshot = Snapshot {
            inspected_disks: vec![inspected_disk(disk_path, DiskBus::Nvme)],
            attached_disks: vec![attached_disk(disk_path, "nvme0:0")],
        };

        assert!(plan(snapshot)?.actions.is_empty());

        Ok(())
    }

    #[test]
    fn disk_on_another_bus_is_moved() -> Result<()> {
        let disk_path = Path::new("/disks/system.vmdk");
        let snapshot = Snapshot {
            inspected_disks: vec![inspected_disk(disk_path, DiskBus::Sata)],
            attached_disks: vec![attached_disk(disk_path, "nvme0:0")],
        };

        let plan = plan(snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Move {
                from,
                to: DiskBus::Sata,
            }] if from == "nvme0:0"
        ));

        Ok(())
    }

    #[test]
    fn unattached_configured_disk_is_attached() -> Result<()> {
        let disk_path = Path::new("/disks/system.vmdk");
        let snapshot = Snapshot {
            inspected_disks: vec![inspected_disk(disk_path, DiskBus::Nvme)],
            attached_disks: Vec::new(),
        };

        let plan = plan(snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Attach {
                path,
                to: DiskBus::Nvme,
            }] if path == disk_path
        ));

        Ok(())
    }

    #[test]
    fn missing_configured_disk_is_created_and_attached() -> Result<()> {
        let disk_path = Path::new("/disks/system.vmdk");
        let mut inspected_disk = inspected_disk(disk_path, DiskBus::Nvme);
        inspected_disk.current_state = None;

        let snapshot = Snapshot {
            inspected_disks: vec![inspected_disk],
            attached_disks: Vec::new(),
        };

        let plan = plan(snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [
                Action::Create {
                    path: create_path,
                    size,
                    format: DiskFormat::Sparse,
                },
                Action::Attach {
                    path: attach_path,
                    to: DiskBus::Nvme,
                },
            ] if create_path == disk_path
                && size.get() == 8
                && attach_path == disk_path
        ));

        Ok(())
    }

    #[test]
    fn unconfigured_attached_disk_is_detached() -> Result<()> {
        let snapshot = Snapshot {
            inspected_disks: Vec::new(),
            attached_disks: vec![attached_disk(Path::new("/disks/system.vmdk"), "nvme0:0")],
        };

        let plan = plan(snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Detach { label }] if label == "nvme0:0"
        ));

        Ok(())
    }

    #[test]
    fn detachments_are_planned_before_attachments() -> Result<()> {
        let new_path = Path::new("/disks/new.vmdk");
        let snapshot = Snapshot {
            inspected_disks: vec![inspected_disk(new_path, DiskBus::Nvme)],
            attached_disks: vec![attached_disk(Path::new("/disks/old.vmdk"), "nvme0:0")],
        };

        let plan = plan(snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [
                Action::Detach { label },
                Action::Attach {
                    path,
                    to: DiskBus::Nvme,
                },
            ] if label == "nvme0:0" && path == new_path
        ));

        Ok(())
    }

    #[test]
    fn disk_aliases_are_matched_by_canonical_path() -> Result<()> {
        let canonical_path = Path::new("/disks/system.vmdk");
        let inspected_disk = InspectedDisk {
            identity_path: canonical_path.to_owned(),
            ..inspected_disk(Path::new("/aliases/system.vmdk"), DiskBus::Nvme)
        };

        let snapshot = Snapshot {
            inspected_disks: vec![inspected_disk],
            attached_disks: vec![attached_disk(canonical_path, "nvme0:0")],
        };

        assert!(plan(snapshot)?.actions.is_empty());

        Ok(())
    }

    #[test]
    fn duplicate_disk_paths_are_rejected() {
        let canonical_path = Path::new("/disks/system.vmdk");
        let snapshot = Snapshot {
            inspected_disks: vec![
                inspected_disk(canonical_path, DiskBus::Nvme),
                InspectedDisk {
                    identity_path: canonical_path.to_owned(),
                    ..inspected_disk(Path::new("/aliases/system.vmdk"), DiskBus::Nvme)
                },
            ],
            attached_disks: Vec::new(),
        };

        let error = plan(snapshot).expect_err("duplicate disk paths should fail");

        assert!(error.to_string().contains("disk paths must be unique"));
    }

    #[test]
    fn disk_on_unsupported_bus_is_moved_to_configured_bus() -> Result<()> {
        let disk_path = Path::new("/disks/system.vmdk");
        let snapshot = Snapshot {
            inspected_disks: vec![inspected_disk(disk_path, DiskBus::Nvme)],
            attached_disks: vec![attached_disk(disk_path, "scsi0:0")],
        };

        let plan = plan(snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Move {
                from,
                to: DiskBus::Nvme,
            }] if from == "scsi0:0"
        ));

        Ok(())
    }

    #[test]
    fn smaller_disk_is_expanded_to_configured_capacity() -> Result<()> {
        let disk_path = Path::new("/disks/system.vmdk");
        let snapshot = Snapshot {
            inspected_disks: vec![inspected_disk_with_capacity(
                disk_path,
                16,
                8 * BYTES_PER_GIB,
            )],
            attached_disks: vec![attached_disk(disk_path, "nvme0:0")],
        };

        let plan = plan(snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Expand { path, size }]
                if path == disk_path && size.get() == 16
        ));

        Ok(())
    }

    #[test]
    fn disk_at_configured_capacity_is_unchanged() -> Result<()> {
        let disk_path = Path::new("/disks/system.vmdk");
        let snapshot = Snapshot {
            inspected_disks: vec![inspected_disk_with_capacity(
                disk_path,
                8,
                8 * BYTES_PER_GIB,
            )],
            attached_disks: vec![attached_disk(disk_path, "nvme0:0")],
        };

        let plan = plan(snapshot)?;

        assert!(plan.actions.is_empty());

        Ok(())
    }

    #[test]
    fn disk_in_another_format_is_converted() -> Result<()> {
        let disk_path = Path::new("/disks/system.vmdk");

        let mut inspected_disk = inspected_disk(disk_path, DiskBus::Nvme);
        inspected_disk.configured.format = DiskFormat::SplitSparse;

        let snapshot = Snapshot {
            inspected_disks: vec![inspected_disk],
            attached_disks: vec![attached_disk(disk_path, "nvme0:0")],
        };

        let plan = plan(snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Convert { path, format: DiskFormat::SplitSparse }]
                if path == disk_path
        ));

        Ok(())
    }

    #[test]
    fn shrinking_disk_is_rejected() {
        let disk_path = Path::new("/disks/system.vmdk");
        let snapshot = Snapshot {
            inspected_disks: vec![inspected_disk_with_capacity(
                disk_path,
                8,
                16 * BYTES_PER_GIB,
            )],
            attached_disks: vec![attached_disk(disk_path, "nvme0:0")],
        };

        let error = plan(snapshot).expect_err("shrinking disk should fail");

        assert!(error.to_string().contains(&format!(
            "cannot resize disk {} below its current capacity",
            disk_path.display()
        )));
    }
}
