use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::config::CONFIG;
use crate::vm::state::State;

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

    let json = duct::cmd!(&CONFIG.vmcli, vmx_path, "disk", "query", "-f", "json")
        .read()
        .context("could not query optical drives")?;

    let query = serde_json::from_str::<VmcliOpticalQuery>(&json)
        .context("could not parse vmcli optical drive JSON")?;

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
    use std::ffi::OsStr;
    use std::fs;

    use crate::vm::test_support::create_vmx;

    use super::*;

    fn set_controller_present(path: &Path) -> Result<()> {
        duct::cmd!(&CONFIG.vmcli, path, "sata", "setpresent", "sata0", "true").run()?;

        Ok(())
    }

    fn set_backing_info(
        path: &Path,
        label: &str,
        backing_type: &str,
        backing_path: impl AsRef<OsStr>,
        client_device: bool,
    ) -> Result<()> {
        duct::cmd!(
            &CONFIG.vmcli,
            path,
            "disk",
            "setbackinginfo",
            label,
            backing_type,
            backing_path.as_ref(),
            client_device.to_string(),
        )
        .run()?;

        Ok(())
    }

    fn set_drive_property(path: &Path, label: &str, command: &str, value: &str) -> Result<()> {
        duct::cmd!(&CONFIG.vmcli, path, "disk", command, label, value).run()?;

        Ok(())
    }

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
        set_controller_present(&vmx_path)?;
        set_backing_info(&vmx_path, label, "cdrom_image", &image_path, false)?;
        set_drive_property(&vmx_path, label, "setstartconnected", "false")?;
        set_drive_property(&vmx_path, label, "setpresent", "true")?;

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

        set_controller_present(&vmx_path)?;
        set_backing_info(&vmx_path, label, "cdrom_raw", "auto detect", true)?;
        set_drive_property(&vmx_path, label, "setpresent", "true")?;

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
