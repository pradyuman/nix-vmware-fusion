use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::CONFIG;
use crate::vm::schema::VirtualDisks;

use super::{AttachedDisk, ConfiguredDisk, Snapshot};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiskQuery {
    disks: Vec<QueriedDisk>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueriedDisk {
    label: String,
    backing_path_name: PathBuf,
}

pub(crate) fn inspect(disks: &VirtualDisks, vmx_path: &Path) -> Result<Snapshot> {
    // Canonicalize paths so equivalent paths and symlinks identify the same disk
    let configured_disks = disks
        .values()
        .map(|disk| {
            let path = disk.path.as_ref();
            let metadata = fs::metadata(path)
                .with_context(|| format!("could not inspect disk {}", path.display()))?;

            ensure!(
                metadata.is_file(),
                "disk path is not a file: {}",
                path.display()
            );

            Ok(ConfiguredDisk {
                path: path.clone(),
                bus: disk.bus,
                canonical_path: fs::canonicalize(path)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Snapshot {
        configured_disks,
        attached_disks: query_attached_disks(vmx_path)?,
    })
}

fn query_attached_disks(vmx_path: &Path) -> Result<Vec<AttachedDisk>> {
    if !vmx_path.try_exists()? {
        return Ok(Vec::new());
    }

    // Query the real VMX so vmcli resolves relative backing paths against its bundle
    let json = duct::cmd!(&CONFIG.vmcli, vmx_path, "disk", "query", "-f", "json")
        .read()
        .context("could not query attached disks")?;

    let parsed =
        serde_json::from_str::<DiskQuery>(&json).context("could not parse vmcli disk JSON")?;

    parsed
        .disks
        .into_iter()
        .map(|disk| {
            Ok(AttachedDisk {
                label: disk.label,
                canonical_path: canonicalize_if_exists(&disk.backing_path_name)?,
            })
        })
        .collect()
}

fn canonicalize_if_exists(path: &Path) -> Result<Option<PathBuf>> {
    Ok(if path.try_exists()? {
        Some(fs::canonicalize(path)?)
    } else {
        None
    })
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;

    use crate::vm::schema::{VirtualDisk, VirtualDiskBus, VirtualDiskPath};

    use super::*;

    fn disks(path: PathBuf) -> VirtualDisks {
        VirtualDisks::from([(
            "disk".to_owned(),
            VirtualDisk {
                path: VirtualDiskPath::try_new(path).expect("valid disk path"),
                bus: VirtualDiskBus::Nvme,
            },
        )])
    }

    #[test]
    fn configured_disk_is_canonicalized() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let disk_path = temp_dir.path().join("system.vmdk");
        let alias_path = temp_dir.path().join("alias.vmdk");
        let vmx_path = temp_dir.path().join("example.vmx");

        fs::write(&disk_path, "")?;
        symlink(&disk_path, &alias_path)?;

        let snapshot = inspect(&disks(alias_path.clone()), &vmx_path)?;

        assert_eq!(snapshot.configured_disks.len(), 1);
        assert_eq!(snapshot.configured_disks[0].path, alias_path);
        assert_eq!(
            snapshot.configured_disks[0].canonical_path,
            fs::canonicalize(disk_path)?
        );
        assert!(snapshot.attached_disks.is_empty());

        Ok(())
    }

    #[test]
    fn missing_configured_disk_is_rejected() {
        let temp_dir = tempfile::tempdir().expect("temporary directory");
        let disk_path = temp_dir.path().join("missing.vmdk");
        let vmx_path = temp_dir.path().join("example.vmx");

        let error = inspect(&disks(disk_path), &vmx_path).expect_err("missing disk should fail");

        assert!(error.to_string().contains("could not inspect disk"));
    }

    #[test]
    fn configured_disk_directory_is_rejected() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let disk_path = temp_dir.path().join("directory.vmdk");
        let vmx_path = temp_dir.path().join("example.vmx");

        fs::create_dir(&disk_path)?;

        let error = inspect(&disks(disk_path), &vmx_path).expect_err("disk directory should fail");

        assert!(error.to_string().contains("disk path is not a file"));

        Ok(())
    }
}
