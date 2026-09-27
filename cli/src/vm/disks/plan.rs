use anyhow::{Context, Result, bail, ensure};
use std::cmp::Ordering;
use std::collections::HashSet;

use crate::vm::schema::{VirtualDisk, VirtualDisks};

use super::{Action, BYTES_PER_GIB, DiskFormat, DiskImage, Plan, Snapshot};

pub(crate) fn plan(disks: &VirtualDisks, snapshot: Snapshot) -> Result<Plan> {
    validate_disk_paths(disks, &snapshot.disk_images)?;

    // Pair each configured disk with the image currently present at its path
    let disk_matches = disks
        .values()
        .map(|disk| {
            let image = snapshot
                .disk_images
                .iter()
                .find(|image| &image.path == disk.path.as_ref());

            (disk, image)
        })
        .collect::<Vec<_>>();

    let attachments = disk_matches.iter().filter_map(|(disk, image)| {
        // Canonical paths let VMX attachments match images reached through aliases
        let attachment = image.and_then(|image| {
            snapshot.disk_attachments.iter().find(|attachment| {
                attachment.canonical_path.as_ref() == Some(&image.canonical_path)
            })
        });

        match attachment {
            Some(attachment) => (attachment.label.bus() != Some(disk.bus)).then(|| Action::Move {
                from: attachment.label.clone(),
                to: disk.bus,
            }),
            None => Some(Action::Attach {
                path: disk.path.as_ref().to_owned(),
                to: disk.bus,
            }),
        }
    });

    let detachments = snapshot
        .disk_attachments
        .iter()
        .filter(|attachment| {
            !snapshot
                .disk_images
                .iter()
                .any(|image| attachment.canonical_path.as_ref() == Some(&image.canonical_path))
        })
        .map(|attachment| Action::Detach {
            label: attachment.label.clone(),
        });

    let creations = disk_matches
        .iter()
        .filter(|(_, image)| image.is_none())
        .map(|(disk, _)| Action::Create {
            path: disk.path.as_ref().to_owned(),
            size: disk.size,
            format: configured_format(disk),
        });

    let expansions = disk_matches
        .iter()
        .filter_map(|(disk, image)| image.map(|image| plan_expansion(disk, image)))
        .filter_map(Result::transpose)
        .collect::<Result<Vec<_>>>()?;

    let conversions = disk_matches.iter().filter_map(|(disk, image)| {
        let format = configured_format(disk);

        image
            .filter(|image| format != image.state.format)
            .map(|_| Action::Convert {
                path: disk.path.as_ref().to_owned(),
                format,
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

fn validate_disk_paths(disks: &VirtualDisks, images: &[DiskImage]) -> Result<()> {
    // Check configured paths directly because missing disk images cannot be canonicalized
    let configured_paths = disks
        .values()
        .map(|disk| disk.path.as_ref())
        .collect::<HashSet<_>>();

    ensure!(
        configured_paths.len() == disks.len(),
        "configured disk paths must be unique"
    );

    // Check canonical paths so aliases cannot configure the same existing disk twice
    let canonical_paths = images
        .iter()
        .map(|image| &image.canonical_path)
        .collect::<HashSet<_>>();

    ensure!(
        canonical_paths.len() == images.len(),
        "configured disk paths must not refer to the same existing disk"
    );

    Ok(())
}

fn plan_expansion(disk: &VirtualDisk, image: &DiskImage) -> Result<Option<Action>> {
    let requested_bytes = disk
        .size
        .get()
        .checked_mul(BYTES_PER_GIB)
        .context("configured disk capacity is too large")?;

    match requested_bytes.cmp(&image.state.capacity_bytes.get()) {
        Ordering::Less => bail!(
            "cannot resize disk {} below its current capacity (requested: {} GiB)",
            disk.path.as_ref().display(),
            disk.size
        ),
        Ordering::Greater => Ok(Some(Action::Expand {
            path: disk.path.as_ref().to_owned(),
            size: disk.size,
        })),
        Ordering::Equal => Ok(None),
    }
}

fn configured_format(disk: &VirtualDisk) -> DiskFormat {
    DiskFormat::from_options(disk.preallocate, disk.split)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;
    use std::path::Path;

    use crate::vm::disks::{DiskAttachment, DiskImage, DiskLabel, DiskState};
    use crate::vm::schema::{DiskBus, DiskPath, VirtualDisk};

    use super::*;

    fn configured_disks(path: &Path, bus: DiskBus) -> VirtualDisks {
        VirtualDisks::from([("primary".to_owned(), configured_disk(path, bus))])
    }

    fn configured_disk(path: &Path, bus: DiskBus) -> VirtualDisk {
        VirtualDisk {
            path: DiskPath::try_new(path.to_owned()).expect("valid disk path"),
            size: NonZeroU64::new(8).expect("non-zero disk capacity"),
            bus,
            preallocate: false,
            split: false,
        }
    }

    fn disk_image(path: &Path) -> DiskImage {
        DiskImage {
            path: path.to_owned(),
            canonical_path: path.to_owned(),
            state: DiskState {
                capacity_bytes: NonZeroU64::new(8 * BYTES_PER_GIB)
                    .expect("non-zero current disk capacity"),
                format: DiskFormat::Sparse,
            },
        }
    }

    fn disk_image_with_capacity(path: &Path, capacity_bytes: u64) -> DiskImage {
        let mut image = disk_image(path);
        image.state.capacity_bytes =
            NonZeroU64::new(capacity_bytes).expect("non-zero current disk capacity");

        image
    }

    fn disk_attachment(path: &Path, label: &str) -> DiskAttachment {
        DiskAttachment {
            label: DiskLabel::new(label),
            canonical_path: Some(path.to_owned()),
        }
    }

    #[test]
    fn disk_on_configured_bus_is_unchanged() -> Result<()> {
        let disk_path = Path::new("/disks/system.vmdk");
        let disks = configured_disks(disk_path, DiskBus::Nvme);
        let snapshot = Snapshot {
            disk_images: vec![disk_image(disk_path)],
            disk_attachments: vec![disk_attachment(disk_path, "nvme0:0")],
        };

        assert!(plan(&disks, snapshot)?.actions.is_empty());

        Ok(())
    }

    #[test]
    fn disk_on_another_bus_is_moved() -> Result<()> {
        let disk_path = Path::new("/disks/system.vmdk");
        let disks = configured_disks(disk_path, DiskBus::Sata);
        let snapshot = Snapshot {
            disk_images: vec![disk_image(disk_path)],
            disk_attachments: vec![disk_attachment(disk_path, "nvme0:0")],
        };

        let plan = plan(&disks, snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Move {
                from,
                to: DiskBus::Sata,
            }] if from.as_ref() == "nvme0:0"
        ));

        Ok(())
    }

    #[test]
    fn disk_on_unsupported_bus_is_moved_to_configured_bus() -> Result<()> {
        let disk_path = Path::new("/disks/system.vmdk");
        let disks = configured_disks(disk_path, DiskBus::Nvme);
        let snapshot = Snapshot {
            disk_images: vec![disk_image(disk_path)],
            disk_attachments: vec![disk_attachment(disk_path, "scsi0:0")],
        };

        let plan = plan(&disks, snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Move {
                from,
                to: DiskBus::Nvme,
            }] if from.as_ref() == "scsi0:0"
        ));

        Ok(())
    }

    #[test]
    fn unattached_configured_disk_is_attached() -> Result<()> {
        let disk_path = Path::new("/disks/system.vmdk");
        let disks = configured_disks(disk_path, DiskBus::Nvme);
        let snapshot = Snapshot {
            disk_images: vec![disk_image(disk_path)],
            disk_attachments: Vec::new(),
        };

        let plan = plan(&disks, snapshot)?;

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
        let disks = configured_disks(disk_path, DiskBus::Nvme);
        let snapshot = Snapshot {
            disk_images: Vec::new(),
            disk_attachments: Vec::new(),
        };

        let plan = plan(&disks, snapshot)?;

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
        let disks = VirtualDisks::new();
        let snapshot = Snapshot {
            disk_images: Vec::new(),
            disk_attachments: vec![disk_attachment(Path::new("/disks/system.vmdk"), "nvme0:0")],
        };

        let plan = plan(&disks, snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Detach { label }] if label.as_ref() == "nvme0:0"
        ));

        Ok(())
    }

    #[test]
    fn detachments_are_planned_before_attachments() -> Result<()> {
        let new_path = Path::new("/disks/new.vmdk");
        let disks = configured_disks(new_path, DiskBus::Nvme);
        let snapshot = Snapshot {
            disk_images: vec![disk_image(new_path)],
            disk_attachments: vec![disk_attachment(Path::new("/disks/old.vmdk"), "nvme0:0")],
        };

        let plan = plan(&disks, snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [
                Action::Detach { label },
                Action::Attach {
                    path,
                    to: DiskBus::Nvme,
                },
            ] if label.as_ref() == "nvme0:0" && path == new_path
        ));

        Ok(())
    }

    #[test]
    fn disk_aliases_are_matched_by_canonical_path() -> Result<()> {
        let alias_path = Path::new("/aliases/system.vmdk");
        let canonical_path = Path::new("/disks/system.vmdk");
        let disks = configured_disks(alias_path, DiskBus::Nvme);
        let image = DiskImage {
            canonical_path: canonical_path.to_owned(),
            ..disk_image(alias_path)
        };

        let snapshot = Snapshot {
            disk_images: vec![image],
            disk_attachments: vec![disk_attachment(canonical_path, "nvme0:0")],
        };

        assert!(plan(&disks, snapshot)?.actions.is_empty());

        Ok(())
    }

    #[test]
    fn duplicate_disk_paths_are_rejected() {
        let alias_path = Path::new("/aliases/system.vmdk");
        let canonical_path = Path::new("/disks/system.vmdk");
        let disks = VirtualDisks::from([
            (
                "primary".to_owned(),
                configured_disk(canonical_path, DiskBus::Nvme),
            ),
            (
                "secondary".to_owned(),
                configured_disk(alias_path, DiskBus::Nvme),
            ),
        ]);
        let snapshot = Snapshot {
            disk_images: vec![
                disk_image(canonical_path),
                DiskImage {
                    canonical_path: canonical_path.to_owned(),
                    ..disk_image(alias_path)
                },
            ],
            disk_attachments: Vec::new(),
        };

        let error = plan(&disks, snapshot).expect_err("duplicate disk paths should fail");

        assert!(
            error
                .to_string()
                .contains("configured disk paths must not refer to the same existing disk")
        );
    }

    #[test]
    fn smaller_disk_is_expanded_to_configured_capacity() -> Result<()> {
        let disk_path = Path::new("/disks/system.vmdk");
        let mut disks = configured_disks(disk_path, DiskBus::Nvme);
        disks.get_mut("primary").expect("configured disk").size =
            NonZeroU64::new(16).expect("non-zero disk capacity");
        let snapshot = Snapshot {
            disk_images: vec![disk_image_with_capacity(disk_path, 8 * BYTES_PER_GIB)],
            disk_attachments: vec![disk_attachment(disk_path, "nvme0:0")],
        };

        let plan = plan(&disks, snapshot)?;

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
        let disks = configured_disks(disk_path, DiskBus::Nvme);
        let snapshot = Snapshot {
            disk_images: vec![disk_image_with_capacity(disk_path, 8 * BYTES_PER_GIB)],
            disk_attachments: vec![disk_attachment(disk_path, "nvme0:0")],
        };

        let plan = plan(&disks, snapshot)?;

        assert!(plan.actions.is_empty());

        Ok(())
    }

    #[test]
    fn disk_in_another_format_is_converted() -> Result<()> {
        let disk_path = Path::new("/disks/system.vmdk");
        let mut disks = configured_disks(disk_path, DiskBus::Nvme);
        disks.get_mut("primary").expect("configured disk").split = true;

        let snapshot = Snapshot {
            disk_images: vec![disk_image(disk_path)],
            disk_attachments: vec![disk_attachment(disk_path, "nvme0:0")],
        };

        let plan = plan(&disks, snapshot)?;

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
        let disks = configured_disks(disk_path, DiskBus::Nvme);
        let snapshot = Snapshot {
            disk_images: vec![disk_image_with_capacity(disk_path, 16 * BYTES_PER_GIB)],
            disk_attachments: vec![disk_attachment(disk_path, "nvme0:0")],
        };

        let error = plan(&disks, snapshot).expect_err("shrinking disk should fail");

        assert!(error.to_string().contains(&format!(
            "cannot resize disk {} below its current capacity",
            disk_path.display()
        )));
    }
}
