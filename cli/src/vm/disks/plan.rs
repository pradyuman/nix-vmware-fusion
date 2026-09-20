use anyhow::{Result, bail};

use crate::vm::schema::DiskBus;

use super::{Action, Plan, Snapshot};

pub(crate) fn plan(snapshot: Snapshot) -> Result<Plan> {
    let declared = snapshot
        .declared_disks
        .iter()
        .map(|disk| {
            let attached = snapshot
                .attached_disks
                .iter()
                .find(|attached| attached.canonical_path.as_ref() == Some(&disk.canonical_path));

            match attached {
                Some(attached) => {
                    let bus = bus_from_label(&attached.label)?;

                    Ok((bus != disk.bus).then(|| Action::Move {
                        from: attached.label.clone(),
                        to: disk.bus,
                    }))
                }
                None => Ok(Some(Action::Attach {
                    path: disk.path.clone(),
                    to: disk.bus,
                })),
            }
        })
        .filter_map(|action| action.transpose());

    let undeclared = snapshot
        .attached_disks
        .iter()
        .filter(|attached| {
            !snapshot
                .declared_disks
                .iter()
                .any(|declared| attached.canonical_path.as_ref() == Some(&declared.canonical_path))
        })
        .map(|attached| Action::Detach {
            label: attached.label.clone(),
        })
        .map(Ok);

    // Detach undeclared disks first so moves and attachments can reuse their labels
    Ok(Plan {
        actions: undeclared.chain(declared).collect::<Result<_>>()?,
    })
}

fn bus_from_label(label: &str) -> Result<DiskBus> {
    if label.starts_with("nvme") {
        Ok(DiskBus::Nvme)
    } else if label.starts_with("sata") {
        Ok(DiskBus::Sata)
    } else {
        bail!("unsupported disk label: {label}")
    }
}
