use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::vm::state::State;
use crate::vm::vmcli;

use super::{OpticalAttachment, OpticalDriveLabel, Snapshot};

#[derive(Deserialize)]
struct VmcliOpticalQuery {
    #[serde(default)]
    cdroms: Vec<VmcliOpticalDrive>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VmcliOpticalDrive {
    label: OpticalDriveLabel,
    backing_type: Option<String>,
    backing_path_name: Option<String>,
    #[serde(default)]
    client_device: bool,
    #[serde(default)]
    start_connected: bool,
}

pub(crate) fn inspect(vmx_path: &Path) -> Result<Snapshot> {
    let bundle_path = vmx_path.parent().context("missing VMX directory")?;

    Ok(Snapshot {
        state: State::load(bundle_path)?,
        optical_attachments: query_optical_attachments(vmx_path)?,
    })
}

fn query_optical_attachments(vmx_path: &Path) -> Result<Vec<OpticalAttachment>> {
    if !vmx_path.try_exists()? {
        return Ok(Vec::new());
    }

    let query = vmcli::query::<VmcliOpticalQuery>(vmx_path, "disk")
        .context("could not query optical drives")?;

    Ok(query
        .cdroms
        .into_iter()
        .map(|drive| OpticalAttachment {
            label: drive.label,
            backing_type: drive.backing_type,
            backing_path: drive
                .backing_path_name
                .filter(|path| !path.is_empty())
                .map(PathBuf::from),
            client_device: drive.client_device,
            start_connected: drive.start_connected,
        })
        .collect())
}

#[cfg(all(test, feature = "vmware-tests"))]
mod tests {
    use std::fs;

    use crate::vm::test_support::create_vmx;

    use super::*;

    #[test]
    fn vmcli_reports_no_optical_attachments() -> Result<()> {
        let (_temp_dir, vmx_path) = create_vmx()?;

        assert!(query_optical_attachments(&vmx_path)?.is_empty());

        Ok(())
    }

    #[test]
    fn vmcli_reports_image_backed_optical_attachment() -> Result<()> {
        let (temp_dir, vmx_path) = create_vmx()?;
        let image_path = temp_dir.path().join("installer.iso");
        let label = "sata0:0";

        fs::write(&image_path, [])?;
        vmcli::enable_controller(&vmx_path, "sata", "sata0")?;
        vmcli::set_backing_info(&vmx_path, label, "cdrom_image", &image_path, false)?;
        vmcli::run(&vmx_path, "disk", "setstartconnected", label, "false")?;
        vmcli::run(&vmx_path, "disk", "setpresent", label, "true")?;

        let attachments = query_optical_attachments(&vmx_path)?;

        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].label, label);
        assert_eq!(attachments[0].backing_type.as_deref(), Some("cdrom_image"));
        assert_eq!(
            attachments[0].backing_path.as_deref(),
            Some(image_path.as_path())
        );
        assert!(!attachments[0].client_device);
        assert!(!attachments[0].start_connected);

        Ok(())
    }

    #[test]
    fn vmcli_reports_client_device_optical_attachment() -> Result<()> {
        let (_temp_dir, vmx_path) = create_vmx()?;
        let label = "sata0:0";

        vmcli::enable_controller(&vmx_path, "sata", "sata0")?;
        vmcli::set_backing_info(&vmx_path, label, "cdrom_raw", "auto detect", true)?;
        vmcli::run(&vmx_path, "disk", "setpresent", label, "true")?;

        let attachments = query_optical_attachments(&vmx_path)?;

        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].label, label);
        assert_eq!(attachments[0].backing_type.as_deref(), Some("cdrom_raw"));
        assert_eq!(
            attachments[0].backing_path.as_deref(),
            Some(Path::new("auto detect"))
        );
        assert!(attachments[0].client_device);

        Ok(())
    }
}
