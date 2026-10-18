use std::path::Path;

use anyhow::{Context, Result};

use super::{Action, Plan, StagedChange};
use crate::vm::schema::{OpticalDrive, OpticalDriveSource};
use crate::vm::state::OpticalDriveBinding;
use crate::vm::vmcli;

pub(crate) fn stage(draft_path: &Path, plan: Plan) -> Result<StagedChange> {
    let Plan { mut state, actions } = plan;

    actions.into_iter().try_for_each(|action| match action {
        Action::Configure { name, label, drive } => {
            vmcli::enable_controller(draft_path, "sata", "sata0")?;

            let label =
                label.map_or_else(|| vmcli::find_first_free(draft_path, "sata", "sata0"), Ok)?;

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

fn configure(path: &Path, label: &str, drive: &OpticalDrive) -> Result<()> {
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
    use crate::vm::optical::inspect;
    use crate::vm::schema::OpticalImagePath;
    use crate::vm::state::State;
    use crate::vm::test_support::create_vmx;

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
}
