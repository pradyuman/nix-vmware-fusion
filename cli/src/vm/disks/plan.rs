use anyhow::{Context, Result, bail, ensure};
use std::cmp::Ordering;
use std::collections::HashSet;

use crate::vm::schema::DiskBus;

use super::{Action, BYTES_PER_GIB, ConfiguredDisk, Plan, Snapshot};

pub(crate) fn plan(snapshot: Snapshot) -> Result<Plan> {
    let canonical_paths = snapshot
        .configured_disks
        .iter()
        .map(|disk| &disk.canonical_path)
        .collect::<HashSet<_>>();

    ensure!(
        canonical_paths.len() == snapshot.configured_disks.len(),
        "disk paths must be unique"
    );

    let configured = snapshot.configured_disks.iter().filter_map(|disk| {
        let attached = snapshot
            .attached_disks
            .iter()
            .find(|attached| attached.canonical_path.as_ref() == Some(&disk.canonical_path));

        match attached {
            Some(attached) => {
                let bus = bus_from_label(&attached.label);

                (bus != Some(disk.bus)).then(|| Action::Move {
                    from: attached.label.clone(),
                    to: disk.bus,
                })
            }
            None => Some(Action::Attach {
                path: disk.path.clone(),
                to: disk.bus,
            }),
        }
    });

    let unconfigured = snapshot
        .attached_disks
        .iter()
        .filter(|attached| {
            !snapshot.configured_disks.iter().any(|configured| {
                attached.canonical_path.as_ref() == Some(&configured.canonical_path)
            })
        })
        .map(|attached| Action::Detach {
            label: attached.label.clone(),
        });

    let expansions = snapshot
        .configured_disks
        .iter()
        .map(plan_expansion)
        .filter_map(Result::transpose)
        .collect::<Result<Vec<_>>>()?;

    let conversions = snapshot
        .configured_disks
        .iter()
        .filter(|disk| disk.format != disk.current_state.format)
        .map(|disk| Action::Convert {
            path: disk.path.clone(),
            format: disk.format,
        });

    // Detach unconfigured disks first so moves and attachments can reuse their labels
    Ok(Plan {
        actions: unconfigured
            .chain(configured)
            .chain(expansions)
            .chain(conversions)
            .collect(),
    })
}

fn plan_expansion(disk: &ConfiguredDisk) -> Result<Option<Action>> {
    let requested_bytes = disk
        .size
        .get()
        .checked_mul(BYTES_PER_GIB)
        .context("configured disk capacity is too large")?;

    match requested_bytes.cmp(&disk.current_state.capacity_bytes.get()) {
        Ordering::Less => bail!(
            "cannot resize disk {} below its current capacity (requested: {} GiB)",
            disk.path.display(),
            disk.size
        ),
        Ordering::Greater => Ok(Some(Action::Expand {
            path: disk.path.clone(),
            size: disk.size,
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

    use crate::vm::disks::{AttachedDisk, ConfiguredDisk, DiskFormat, DiskState};

    use super::*;

    fn configured(path: &str, bus: DiskBus) -> ConfiguredDisk {
        ConfiguredDisk {
            path: path.into(),
            size: NonZeroU64::new(8).expect("non-zero disk capacity"),
            bus,
            format: DiskFormat::Sparse,
            canonical_path: path.into(),
            current_state: DiskState {
                capacity_bytes: NonZeroU64::new(8 * BYTES_PER_GIB)
                    .expect("non-zero current disk capacity"),
                format: DiskFormat::Sparse,
            },
        }
    }

    fn configured_with_capacity(path: &str, size: u64, current_bytes: u64) -> ConfiguredDisk {
        let mut disk = configured(path, DiskBus::Nvme);
        disk.size = NonZeroU64::new(size).expect("non-zero disk capacity");
        disk.current_state.capacity_bytes =
            NonZeroU64::new(current_bytes).expect("non-zero current disk capacity");

        disk
    }

    fn attached(path: &str, label: &str) -> AttachedDisk {
        AttachedDisk {
            label: label.to_owned(),
            canonical_path: Some(path.into()),
        }
    }

    #[test]
    fn disk_on_configured_bus_is_unchanged() -> Result<()> {
        let disk_path = "/disks/system.vmdk";
        let snapshot = Snapshot {
            configured_disks: vec![configured(disk_path, DiskBus::Nvme)],
            attached_disks: vec![attached(disk_path, "nvme0:0")],
        };

        assert!(plan(snapshot)?.actions.is_empty());

        Ok(())
    }

    #[test]
    fn disk_on_another_bus_is_moved() -> Result<()> {
        let disk_path = "/disks/system.vmdk";
        let snapshot = Snapshot {
            configured_disks: vec![configured(disk_path, DiskBus::Sata)],
            attached_disks: vec![attached(disk_path, "nvme0:0")],
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
        let disk_path = "/disks/system.vmdk";
        let snapshot = Snapshot {
            configured_disks: vec![configured(disk_path, DiskBus::Nvme)],
            attached_disks: Vec::new(),
        };

        let plan = plan(snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Attach {
                path,
                to: DiskBus::Nvme,
            }] if path == Path::new(disk_path)
        ));

        Ok(())
    }

    #[test]
    fn unconfigured_attached_disk_is_detached() -> Result<()> {
        let snapshot = Snapshot {
            configured_disks: Vec::new(),
            attached_disks: vec![attached("/disks/system.vmdk", "nvme0:0")],
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
        let new_path = "/disks/new.vmdk";
        let snapshot = Snapshot {
            configured_disks: vec![configured(new_path, DiskBus::Nvme)],
            attached_disks: vec![attached("/disks/old.vmdk", "nvme0:0")],
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
            ] if label == "nvme0:0" && path == Path::new(new_path)
        ));

        Ok(())
    }

    #[test]
    fn disk_aliases_are_matched_by_canonical_path() -> Result<()> {
        let canonical_path = "/disks/system.vmdk";
        let configured_disk = ConfiguredDisk {
            canonical_path: canonical_path.into(),
            ..configured("/aliases/system.vmdk", DiskBus::Nvme)
        };
        let snapshot = Snapshot {
            configured_disks: vec![configured_disk],
            attached_disks: vec![attached(canonical_path, "nvme0:0")],
        };

        assert!(plan(snapshot)?.actions.is_empty());

        Ok(())
    }

    #[test]
    fn duplicate_canonical_paths_are_rejected() {
        let canonical_path = "/disks/system.vmdk";
        let snapshot = Snapshot {
            configured_disks: vec![
                configured(canonical_path, DiskBus::Nvme),
                ConfiguredDisk {
                    canonical_path: canonical_path.into(),
                    ..configured("/aliases/system.vmdk", DiskBus::Nvme)
                },
            ],
            attached_disks: Vec::new(),
        };

        let error = plan(snapshot).expect_err("duplicate disk paths should fail");

        assert!(error.to_string().contains("disk paths must be unique"));
    }

    #[test]
    fn disk_on_unsupported_bus_is_moved_to_configured_bus() -> Result<()> {
        let disk_path = "/disks/system.vmdk";
        let snapshot = Snapshot {
            configured_disks: vec![configured(disk_path, DiskBus::Nvme)],
            attached_disks: vec![attached(disk_path, "scsi0:0")],
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
        let disk_path = "/disks/system.vmdk";
        let snapshot = Snapshot {
            configured_disks: vec![configured_with_capacity(disk_path, 16, 8 * BYTES_PER_GIB)],
            attached_disks: vec![attached(disk_path, "nvme0:0")],
        };

        let plan = plan(snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Expand { path, size }]
                if path == Path::new(disk_path) && size.get() == 16
        ));

        Ok(())
    }

    #[test]
    fn disk_at_configured_capacity_is_unchanged() -> Result<()> {
        let disk_path = "/disks/system.vmdk";
        let snapshot = Snapshot {
            configured_disks: vec![configured_with_capacity(disk_path, 8, 8 * BYTES_PER_GIB)],
            attached_disks: vec![attached(disk_path, "nvme0:0")],
        };

        let plan = plan(snapshot)?;

        assert!(plan.actions.is_empty());

        Ok(())
    }

    #[test]
    fn disk_in_another_format_is_converted() -> Result<()> {
        let disk_path = "/disks/system.vmdk";

        let mut configured_disk = configured(disk_path, DiskBus::Nvme);
        configured_disk.format = DiskFormat::SplitSparse;
        configured_disk.current_state.format = DiskFormat::Sparse;

        let snapshot = Snapshot {
            configured_disks: vec![configured_disk],
            attached_disks: vec![attached(disk_path, "nvme0:0")],
        };

        let plan = plan(snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Convert { path, format: DiskFormat::SplitSparse }]
                if path == Path::new(disk_path)
        ));

        Ok(())
    }

    #[test]
    fn shrinking_disk_is_rejected() {
        let disk_path = "/disks/system.vmdk";
        let snapshot = Snapshot {
            configured_disks: vec![configured_with_capacity(disk_path, 8, 16 * BYTES_PER_GIB)],
            attached_disks: vec![attached(disk_path, "nvme0:0")],
        };

        let error = plan(snapshot).expect_err("shrinking disk should fail");

        assert!(error.to_string().contains(&format!(
            "cannot resize disk {disk_path} below its current capacity"
        )));
    }
}
