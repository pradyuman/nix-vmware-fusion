use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::fs;
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};

use crate::config::CONFIG;
use crate::vm::schema::VirtualDisks;

use super::{AttachedDisk, ConfiguredDisk, DiskFormat, DiskState, Snapshot};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VmcliDiskQuery {
    disks: Vec<VmcliDisk>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VmcliDisk {
    label: String,
    backing_path_name: PathBuf,
}

pub(crate) fn inspect(vmx_path: &Path, disks: &VirtualDisks) -> Result<Snapshot> {
    Ok(Snapshot {
        configured_disks: inspect_configured_disks(disks)?,
        attached_disks: query_attached_disks(vmx_path)?,
    })
}

fn inspect_configured_disks(disks: &VirtualDisks) -> Result<Vec<ConfiguredDisk>> {
    // Canonicalize paths so equivalent paths and symlinks identify the same disk
    disks
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
                size: disk.size,
                bus: disk.bus,
                format: DiskFormat::from_options(disk.preallocate, disk.split),
                canonical_path: fs::canonicalize(path)?,
                current_state: DiskState {
                    capacity_bytes: read_capacity(path).with_context(|| {
                        format!("could not read disk capacity from {}", path.display())
                    })?,
                    format: read_format(path).with_context(|| {
                        format!("could not read disk format from {}", path.display())
                    })?,
                },
            })
        })
        .collect()
}

pub(super) fn read_format(path: &Path) -> Result<DiskFormat> {
    let json = duct::cmd!(&CONFIG.qemu_img, "info", "--output=json", path)
        .read()
        .context("could not run qemu-img")?;

    let info = serde_json::from_str::<serde_json::Value>(&json)
        .context("could not parse qemu-img disk information")?;
    let create_type = info
        .pointer("/format-specific/data/create-type")
        .and_then(serde_json::Value::as_str)
        .context("qemu-img did not report a VMDK create type")?;

    match create_type {
        "monolithicSparse" => Ok(DiskFormat::Sparse),
        "twoGbMaxExtentSparse" => Ok(DiskFormat::SplitSparse),
        "monolithicFlat" => Ok(DiskFormat::Preallocated),
        "twoGbMaxExtentFlat" => Ok(DiskFormat::SplitPreallocated),
        create_type => bail!("unsupported VMDK format: {create_type}"),
    }
}

fn read_capacity(path: &Path) -> Result<NonZeroU64> {
    let path = path.to_str().context("disk path is not valid UTF-8")?;
    ensure!(
        !path.contains(['"', '\n', '\r']),
        "disk path cannot be represented by vmware-vmdkserver"
    );

    let commands = format!("open -ro \"{path}\"\nstat\nclose\n");
    let output = duct::cmd!(&CONFIG.vmdk_server)
        .stdin_bytes(commands)
        .stdout_capture()
        .stderr_capture()
        .unchecked()
        .run()
        .context("could not run vmware-vmdkserver")?;

    if !output.status.success() {
        bail!(
            "vmware-vmdkserver could not inspect disk: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    String::from_utf8(output.stdout)?
        .trim()
        .parse()
        .with_context(|| {
            format!(
                "vmware-vmdkserver returned an invalid disk capacity: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )
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
        serde_json::from_str::<VmcliDiskQuery>(&json).context("could not parse vmcli disk JSON")?;

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

#[cfg(all(test, feature = "vmware-tests"))]
mod tests {
    use super::*;

    #[test]
    fn reads_vmware_disk_formats() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let disk_formats = [
            ("0", DiskFormat::Sparse),
            ("1", DiskFormat::SplitSparse),
            ("2", DiskFormat::Preallocated),
            ("3", DiskFormat::SplitPreallocated),
        ];

        disk_formats
            .into_iter()
            .try_for_each(|(disk_type, expected_format)| -> Result<()> {
                let path = temp_dir.path().join(format!("type-{disk_type}.vmdk"));

                duct::cmd!(
                    &CONFIG.vdisk_manager,
                    "-c",
                    "-s",
                    "1MB",
                    "-a",
                    "lsilogic",
                    "-t",
                    disk_type,
                    "-q",
                    &path
                )
                .run()?;

                assert_eq!(read_format(&path)?, expected_format);

                Ok(())
            })
    }
}
