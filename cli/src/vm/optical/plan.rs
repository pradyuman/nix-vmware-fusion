use std::collections::BTreeMap;

use crate::vm::schema::{OpticalDrive, OpticalDriveSource, OpticalDrives};

use super::{Action, OpticalAttachment, Plan, Snapshot};

pub(crate) fn plan(drives: &OpticalDrives, snapshot: Snapshot) -> Plan {
    let Snapshot {
        mut state,
        optical_attachments,
    } = snapshot;

    let attachments = optical_attachments
        .into_iter()
        .map(|attachment| (attachment.label.clone(), attachment))
        .collect::<BTreeMap<_, _>>();

    let (retained_bindings, removed_bindings) = state
        .optical_drives
        .into_iter()
        .partition(|(name, _)| drives.contains_key(name));
    state.optical_drives = retained_bindings;

    // Remove only drives previously claimed in our state file.
    let removals = removed_bindings
        .into_values()
        .filter(|binding| attachments.contains_key(&binding.label))
        .map(|binding| Action::Detach {
            label: binding.label,
        });

    let configurations = drives.iter().filter_map(|(name, drive)| {
        let label = state
            .optical_drives
            .get(name)
            .map(|binding| binding.label.clone());

        let matches = label
            .as_ref()
            .and_then(|label| attachments.get(label))
            .is_some_and(|attachment| matches_configuration(attachment, drive));

        (!matches).then(|| Action::Configure {
            name: name.clone(),
            label,
            drive: drive.clone(),
        })
    });

    let actions = removals.chain(configurations).collect();

    Plan { state, actions }
}

fn matches_configuration(attached: &OpticalAttachment, configured: &OpticalDrive) -> bool {
    let source_matches = match &configured.source {
        OpticalDriveSource::Image { path } => {
            attached.backing_type.as_deref() == Some("cdrom_image")
                && attached.backing_path.as_deref() == Some(path.as_ref())
                && !attached.client_device
        }
    };

    source_matches && attached.start_connected == configured.start_connected
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::vm::schema::OpticalImagePath;
    use crate::vm::state::{OpticalDriveBinding, State};

    use super::*;

    fn image_drive(path: &str) -> OpticalDrive {
        OpticalDrive {
            source: OpticalDriveSource::Image {
                path: OpticalImagePath::try_new(PathBuf::from(path))
                    .expect("valid optical image path"),
            },
            start_connected: true,
        }
    }

    fn image_attachment(label: &str, path: &str) -> OpticalAttachment {
        OpticalAttachment {
            label: label.to_owned(),
            backing_type: Some("cdrom_image".to_owned()),
            backing_path: Some(path.into()),
            client_device: false,
            start_connected: true,
        }
    }

    fn managed_state(name: &str, label: &str) -> State {
        let mut state = State::default();
        state.optical_drives.insert(
            name.to_owned(),
            OpticalDriveBinding {
                label: label.to_owned(),
            },
        );

        state
    }

    fn managed_snapshot(name: &str, attachment: OpticalAttachment) -> Snapshot {
        Snapshot {
            state: managed_state(name, &attachment.label),
            optical_attachments: vec![attachment],
        }
    }

    #[test]
    fn matching_managed_drive_is_unchanged() {
        let path = "/images/installer.iso";
        let drives = OpticalDrives::from([("installer".to_owned(), image_drive(path))]);
        let snapshot = managed_snapshot("installer", image_attachment("sata0:0", path));

        let plan = plan(&drives, snapshot);

        assert!(plan.actions.is_empty());
        assert_eq!(
            plan.state
                .optical_drives
                .get("installer")
                .expect("retained installer binding")
                .label,
            "sata0:0"
        );
    }

    #[test]
    fn unmanaged_drive_is_ignored() {
        let snapshot = Snapshot {
            state: State::default(),
            optical_attachments: vec![image_attachment("sata0:0", "/images/unmanaged.iso")],
        };

        let plan = plan(&OpticalDrives::new(), snapshot);

        assert!(plan.actions.is_empty());
        assert!(plan.state.optical_drives.is_empty());
    }

    #[test]
    fn changed_managed_drive_is_configured() {
        let drives =
            OpticalDrives::from([("installer".to_owned(), image_drive("/images/new.iso"))]);
        let snapshot =
            managed_snapshot("installer", image_attachment("sata0:0", "/images/old.iso"));

        let plan = plan(&drives, snapshot);

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Configure {
                name,
                label: Some(label),
                drive,
            }] if name == "installer"
                && label == "sata0:0"
                && matches!(
                    &drive.source,
                    OpticalDriveSource::Image { path }
                        if path.as_ref() == Path::new("/images/new.iso")
                )
        ));
    }

    #[test]
    fn new_drive_is_configured_without_a_label() {
        let drives =
            OpticalDrives::from([("installer".to_owned(), image_drive("/images/installer.iso"))]);
        let snapshot = Snapshot {
            state: State::default(),
            optical_attachments: Vec::new(),
        };

        let plan = plan(&drives, snapshot);

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Configure { name, label: None, .. }] if name == "installer"
        ));
    }

    #[test]
    fn missing_managed_attachment_is_configured_at_its_known_label() {
        let drives =
            OpticalDrives::from([("installer".to_owned(), image_drive("/images/installer.iso"))]);
        let snapshot = Snapshot {
            state: managed_state("installer", "sata0:0"),
            optical_attachments: Vec::new(),
        };

        let plan = plan(&drives, snapshot);

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Configure {
                name,
                label: Some(label),
                ..
            }] if name == "installer" && label == "sata0:0"
        ));
    }

    #[test]
    fn removed_managed_drive_is_detached() {
        let snapshot = managed_snapshot(
            "installer",
            image_attachment("sata0:0", "/images/installer.iso"),
        );

        let plan = plan(&OpticalDrives::new(), snapshot);

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Detach { label }] if label == "sata0:0"
        ));
        assert!(plan.state.optical_drives.is_empty());
    }
}
