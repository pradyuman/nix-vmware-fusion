use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use super::{Action, OpticalDriveLabel, Plan, StagedChange};
use crate::vm::schema::{OpticalDrive, OpticalDriveSource};
use crate::vm::state::OpticalDriveBinding;
use crate::vm::vmcli;

#[derive(Deserialize)]
struct VmcliDiskLabelQuery {
    #[serde(default)]
    disks: Vec<VmcliDiskLabel>,
}

#[derive(Deserialize)]
struct VmcliDiskLabel {
    label: OpticalDriveLabel,
}

pub(crate) fn stage(draft_path: &Path, plan: Plan) -> Result<StagedChange> {
    let Plan { mut state, actions } = plan;

    actions.into_iter().try_for_each(|action| match action {
        Action::Configure { name, label, drive } => {
            vmcli::enable_controller(draft_path, "sata", "sata0")?;

            let label = match label {
                Some(label) if !holds_disk(draft_path, &label)? => label,
                _ => vmcli::find_first_free(draft_path, "sata", "sata0")?,
            };

            configure(draft_path, &label, &drive)
                .with_context(|| format!("could not configure optical drive {label}"))?;
            state
                .optical_drives
                .insert(name, OpticalDriveBinding { label });

            Ok(())
        }
        Action::Detach { label } => vmcli::purge(draft_path, "disk", &label)
            .with_context(|| format!("could not detach optical drive {label}")),
    })?;

    Ok(StagedChange { state })
}

// Query the evolving draft so disks attached earlier in this apply cannot be overwritten
fn holds_disk(draft_path: &Path, label: &OpticalDriveLabel) -> Result<bool> {
    let query = vmcli::query::<VmcliDiskLabelQuery>(draft_path, "disk")
        .with_context(|| format!("could not check whether a disk holds {label}"))?;

    Ok(query.disks.iter().any(|disk| &disk.label == label))
}

fn configure(path: &Path, label: &OpticalDriveLabel, drive: &OpticalDrive) -> Result<()> {
    match &drive.source {
        OpticalDriveSource::Image { path: image_path } => {
            vmcli::set_backing_info(path, label, "cdrom_image", image_path.as_ref(), false)?;
        }
    }

    vmcli::run(
        path,
        "disk",
        "setstartconnected",
        label,
        drive.start_connected.to_string(),
    )?;
    vmcli::run(path, "disk", "setpresent", label, "true")?;

    Ok(())
}

#[cfg(all(test, feature = "vmware-tests"))]
mod tests {
    use std::fs;

    use super::*;
    use crate::vm::disk;
    use crate::vm::optical::inspect;
    use crate::vm::schema::{OpticalImagePath, VirtualDisks};
    use crate::vm::state::State;
    use crate::vm::test_support::{create_vmdk, create_vmx};

    #[test]
    fn vmcli_configures_and_detaches_optical_drive() -> Result<()> {
        let (temp_dir, vmx_path) = create_vmx()?;
        let image_path = temp_dir.path().join("installer.iso");
        let name = "installer";

        fs::write(&image_path, [])?;

        let staged = stage(
            &vmx_path,
            Plan {
                state: State::default(),
                actions: vec![Action::Configure {
                    name: name.to_owned(),
                    label: None,
                    drive: OpticalDrive {
                        source: OpticalDriveSource::Image {
                            path: OpticalImagePath::try_new(image_path.clone()).unwrap(),
                        },
                        start_connected: false,
                    },
                }],
            },
        )?;

        let label = &staged
            .state
            .optical_drives
            .get(name)
            .expect("assigned optical drive label")
            .label;

        let snapshot = inspect(&vmx_path)?;
        let attachment = snapshot
            .optical_attachments
            .iter()
            .find(|attachment| &attachment.label == label)
            .expect("configured optical attachment");

        assert_eq!(attachment.backing_type.as_deref(), Some("cdrom_image"));
        assert_eq!(
            attachment.backing_path.as_deref(),
            Some(image_path.as_path())
        );
        assert!(!attachment.client_device);
        assert!(!attachment.start_connected);

        stage(
            &vmx_path,
            Plan {
                state: State::default(),
                actions: vec![Action::Detach {
                    label: label.clone(),
                }],
            },
        )?;

        assert!(inspect(&vmx_path)?.optical_attachments.is_empty());

        Ok(())
    }

    #[test]
    fn remembered_slot_holding_a_disk_is_not_reused() -> Result<()> {
        let (temp_dir, vmx_path) = create_vmx()?;
        let disk_path = temp_dir.path().join("data.vmdk");
        let image_path = temp_dir.path().join("installer.iso");
        let disk_label = OpticalDriveLabel::new("sata0:0");

        fs::write(&image_path, [])?;
        create_vmdk(&disk_path, "1MB")?;

        // Attach a disk where the optical drive used to be
        vmcli::enable_controller(&vmx_path, "sata", "sata0")?;
        vmcli::set_backing_info(&vmx_path, &disk_label, "disk", &disk_path, false)?;
        vmcli::run(&vmx_path, "disk", "setpresent", &disk_label, "true")?;

        let staged = stage(
            &vmx_path,
            Plan {
                state: State::default(),
                actions: vec![Action::Configure {
                    name: "installer".to_owned(),
                    label: Some(disk_label.clone()),
                    drive: OpticalDrive {
                        source: OpticalDriveSource::Image {
                            path: OpticalImagePath::try_new(image_path).unwrap(),
                        },
                        start_connected: true,
                    },
                }],
            },
        )?;

        let label = &staged
            .state
            .optical_drives
            .get("installer")
            .expect("assigned optical drive label")
            .label;

        let disk_snapshot = disk::inspect(&vmx_path, &VirtualDisks::new())?;
        let [disk] = disk_snapshot.disk_attachments.as_slice() else {
            panic!("expected one disk attachment");
        };

        assert_ne!(label, &disk_label);
        assert_eq!(disk.label.as_ref(), disk_label.as_ref());
        assert_eq!(disk.canonical_path, Some(fs::canonicalize(&disk_path)?));

        Ok(())
    }
}
