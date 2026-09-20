use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::CONFIG;
use crate::vm::schema::VirtualMachine;

use super::{AttachedDisk, DeclaredDisk, Snapshot};

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

pub(crate) fn inspect(schema: &VirtualMachine, vmx_path: &Path) -> Result<Snapshot> {
    // Canonicalize paths so equivalent paths and symlinks identify the same disk
    let declared_disks = schema
        .disks
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

            Ok(DeclaredDisk {
                path: path.clone(),
                bus: disk.bus,
                canonical_path: fs::canonicalize(path)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Snapshot {
        declared_disks,
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
