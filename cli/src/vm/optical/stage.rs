use std::ffi::OsStr;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::config::CONFIG;
use crate::vm::schema::{OpticalDrive, OpticalDriveSource};
use crate::vm::state::OpticalDriveBinding;

use super::{Action, OpticalDriveLabel, Plan, StagedChange};

#[derive(Deserialize)]
struct FreeOpticalDriveLabel {
    #[serde(rename = "FindFirstFree")]
    label: OpticalDriveLabel,
}

pub(crate) fn stage(draft_path: &Path, plan: Plan) -> Result<StagedChange> {
    let Plan { mut state, actions } = plan;

    actions.into_iter().try_for_each(|action| match action {
        Action::Configure { name, label, drive } => {
            enable_sata_controller(draft_path)?;

            let label = label.map_or_else(|| find_first_free_label(draft_path), Ok)?;

            configure(draft_path, &label, &drive)?;
            state
                .optical_drives
                .insert(name, OpticalDriveBinding { label });

            Ok(())
        }
        Action::Detach { label } => duct::cmd!(&CONFIG.vmcli, draft_path, "disk", "purge", &label)
            .run()
            .with_context(|| format!("could not detach optical drive {label}"))
            .map(|_| ()),
    })?;

    Ok(StagedChange { state })
}

fn enable_sata_controller(path: &Path) -> Result<()> {
    duct::cmd!(&CONFIG.vmcli, path, "sata", "setpresent", "sata0", "true")
        .run()
        .context("could not enable sata0")?;

    Ok(())
}

fn find_first_free_label(path: &Path) -> Result<OpticalDriveLabel> {
    let json = duct::cmd!(
        &CONFIG.vmcli,
        path,
        "sata",
        "findfirstfree",
        "sata0",
        "-f",
        "json"
    )
    .read()
    .context("could not find a free optical drive label")?;

    Ok(serde_json::from_str::<FreeOpticalDriveLabel>(&json)
        .context("could not parse vmcli optical drive label JSON")?
        .label)
}

fn configure(path: &Path, label: &str, drive: &OpticalDrive) -> Result<()> {
    match &drive.source {
        OpticalDriveSource::Image { path: image_path } => {
            set_backing_info(path, label, image_path.as_ref())?;
        }
    }

    set(
        path,
        label,
        "setstartconnected",
        drive.start_connected.to_string(),
    )?;
    set(path, label, "setpresent", "true")?;

    Ok(())
}

fn set_backing_info(path: &Path, label: &str, image_path: &Path) -> Result<()> {
    duct::cmd!(
        &CONFIG.vmcli,
        path,
        "disk",
        "setbackinginfo",
        label,
        "cdrom_image",
        image_path,
        "false"
    )
    .run()
    .with_context(|| format!("could not configure backing for optical drive {label}"))?;

    Ok(())
}

fn set(path: &Path, label: &str, command: &str, value: impl AsRef<OsStr>) -> Result<()> {
    duct::cmd!(&CONFIG.vmcli, path, "disk", command, label, value.as_ref())
        .run()
        .with_context(|| format!("could not run {command} for optical drive {label}"))?;

    Ok(())
}

#[cfg(all(test, feature = "vmware-tests"))]
mod tests {
    use std::fs;

    use crate::vm::optical::inspect;
    use crate::vm::schema::OpticalImagePath;
    use crate::vm::state::State;
    use crate::vm::test_support::create_vmx;

    use super::*;

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
