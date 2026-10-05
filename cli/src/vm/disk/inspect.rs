use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::CONFIG;
use crate::vm::schema::VirtualDisks;

use super::{DiskAttachment, DiskFormat, DiskImage, DiskLabel, DiskState, Snapshot};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VmcliDiskQuery {
    disks: Vec<VmcliDisk>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VmcliDisk {
    label: DiskLabel,
    backing_path_name: PathBuf,
}

pub(crate) fn inspect(vmx_path: &Path, disks: &VirtualDisks) -> Result<Snapshot> {
    Ok(Snapshot {
        disk_images: inspect_disks(disks)?,
        disk_attachments: query_disk_attachments(vmx_path)?,
    })
}

fn inspect_disks(disks: &VirtualDisks) -> Result<Vec<DiskImage>> {
    disks
        .values()
        .filter_map(|disk| inspect_disk(disk.path.as_ref()).transpose())
        .collect()
}

fn inspect_disk(path: &Path) -> Result<Option<DiskImage>> {
    match fs::metadata(path) {
        Ok(metadata) => {
            ensure!(
                metadata.is_file(),
                "disk path is not a file: {}",
                path.display()
            );

            Ok(Some(DiskImage {
                path: path.to_owned(),
                canonical_path: fs::canonicalize(path)?,
                state: read_state(path)
                    .with_context(|| format!("could not inspect disk {}", path.display()))?,
            }))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => {
            Err(error).with_context(|| format!("could not inspect disk {}", path.display()))
        }
    }
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

fn query_disk_attachments(vmx_path: &Path) -> Result<Vec<DiskAttachment>> {
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
            Ok(DiskAttachment {
                label: disk.label,
                canonical_path: canonicalize_if_exists(&disk.backing_path_name)?,
            })
        })
        .collect()
}

fn canonicalize_if_exists(path: &Path) -> Result<Option<PathBuf>> {
    Ok(path
        .try_exists()?
        .then(|| fs::canonicalize(path))
        .transpose()?)
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
                        capacity_bytes: std::num::NonZeroU64::new(1024_u64.pow(2)).unwrap(),
                        format: expected_format,
                    }
                );

                Ok(())
            })
    }
}
