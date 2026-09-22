use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::CONFIG;
use crate::vm::schema::{VirtualDisk, VirtualDisks};

use super::{AttachedDisk, ConfiguredDisk, DiskFormat, DiskState, InspectedDisk, Snapshot};

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
        inspected_disks: inspect_disks(disks)?,
        attached_disks: query_attached_disks(vmx_path)?,
    })
}

fn inspect_disks(disks: &VirtualDisks) -> Result<Vec<InspectedDisk>> {
    disks.values().map(inspect_disk).collect()
}

fn inspect_disk(disk: &VirtualDisk) -> Result<InspectedDisk> {
    let path = disk.path.as_ref();
    let current_state = match fs::metadata(path) {
        Ok(metadata) => {
            ensure!(
                metadata.is_file(),
                "disk path is not a file: {}",
                path.display()
            );

            Some(
                read_state(path)
                    .with_context(|| format!("could not inspect disk {}", path.display()))?,
            )
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(error)
                .with_context(|| format!("could not inspect disk {}", path.display()));
        }
    };

    // Canonicalize existing paths so aliases and symlinks identify the same disk
    let identity_path = if current_state.is_some() {
        fs::canonicalize(path)?
    } else {
        path.clone()
    };

    Ok(InspectedDisk {
        configured: ConfiguredDisk {
            path: path.clone(),
            size: disk.size,
            bus: disk.bus,
            format: DiskFormat::from_options(disk.preallocate, disk.split),
        },
        identity_path,
        current_state,
    })
}

pub(super) fn read_state(path: &Path) -> Result<DiskState> {
    let json = duct::cmd!(&CONFIG.qemu_img, "info", "--output=json", path)
        .read()
        .context("could not run qemu-img")?;

    let info = serde_json::from_str::<serde_json::Value>(&json)
        .context("could not parse qemu-img disk information")?;
    let create_type = info
        .pointer("/format-specific/data/create-type")
        .and_then(serde_json::Value::as_str)
        .context("qemu-img did not report a VMDK create type")?;
    let format = match create_type {
        "monolithicSparse" => DiskFormat::Sparse,
        "twoGbMaxExtentSparse" => DiskFormat::SplitSparse,
        "monolithicFlat" => DiskFormat::Preallocated,
        "twoGbMaxExtentFlat" => DiskFormat::SplitPreallocated,
        create_type => bail!("unsupported VMDK format: {create_type}"),
    };
    let capacity_bytes = info
        .get("virtual-size")
        .and_then(serde_json::Value::as_u64)
        .and_then(std::num::NonZeroU64::new)
        .context("qemu-img did not report a non-zero virtual disk capacity")?;

    Ok(DiskState {
        capacity_bytes,
        format,
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
    fn reads_vmware_disk_state() -> Result<()> {
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

                assert_eq!(
                    read_state(&path)?,
                    DiskState {
                        capacity_bytes: std::num::NonZeroU64::new(1024_u64.pow(2))
                            .expect("non-zero disk capacity"),
                        format: expected_format,
                    }
                );

                Ok(())
            })
    }
}
