use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use super::{ObservedSharedFolder, SharedFolderLabel, Snapshot};
use crate::vm::vmcli;

#[derive(Deserialize)]
struct VmcliSharedFolderQuery {
    folders: Vec<VmcliSharedFolder>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VmcliSharedFolder {
    label: SharedFolderLabel,
    guest_name: String,
    host_path: PathBuf,
    present: bool,
    enabled: bool,
    read_access: bool,
    write_access: bool,
}

pub(crate) fn inspect(vmx_path: &Path) -> Result<Snapshot> {
    Ok(Snapshot {
        shared_folders: query_shared_folders(vmx_path)?,
    })
}

fn query_shared_folders(vmx_path: &Path) -> Result<Vec<ObservedSharedFolder>> {
    if !vmx_path.try_exists()? {
        return Ok(Vec::new());
    }

    let query = vmcli::query::<VmcliSharedFolderQuery>(vmx_path, "HGFS")
        .context("could not query shared folders")?;

    Ok(query
        .folders
        .into_iter()
        .map(|folder| ObservedSharedFolder {
            label: folder.label,
            guest_name: folder.guest_name,
            host_path: folder.host_path,
            present: folder.present,
            enabled: folder.enabled,
            read_access: folder.read_access,
            write_access: folder.write_access,
        })
        .collect())
}

#[cfg(all(test, feature = "vmware-tests"))]
mod tests {
    use std::fs;

    use super::*;
    use crate::vm::test_support::create_vmx;

    #[test]
    fn vmcli_reports_no_shared_folders() -> Result<()> {
        let (_temp_dir, vmx_path) = create_vmx()?;

        assert!(inspect(&vmx_path)?.shared_folders.is_empty());

        Ok(())
    }

    #[test]
    fn vmcli_reports_shared_folder() -> Result<()> {
        let (temp_dir, vmx_path) = create_vmx()?;
        let host_path = temp_dir.path().join("projects");
        let label = "sharedFolder0";

        fs::create_dir(&host_path)?;
        vmcli::run(&vmx_path, "HGFS", "SetPresent", label, "true")?;
        vmcli::run(&vmx_path, "HGFS", "SetGuestName", label, "projects")?;
        vmcli::run(&vmx_path, "HGFS", "SetHostPath", label, &host_path)?;
        vmcli::run(&vmx_path, "HGFS", "SetReadAccess", label, "true")?;
        vmcli::run(&vmx_path, "HGFS", "SetWriteAccess", label, "false")?;
        vmcli::run(&vmx_path, "HGFS", "SetEnabled", label, "true")?;

        let snapshot = inspect(&vmx_path)?;

        assert_eq!(snapshot.shared_folders.len(), 1);

        let folder = &snapshot.shared_folders[0];
        assert_eq!(folder.label.as_ref(), label);
        assert_eq!(folder.guest_name, "projects");
        assert_eq!(folder.host_path, host_path);
        assert!(folder.present);
        assert!(folder.enabled);
        assert!(folder.read_access);
        assert!(!folder.write_access);

        Ok(())
    }
}
