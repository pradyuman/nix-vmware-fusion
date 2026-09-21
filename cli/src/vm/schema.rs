use nutype::nutype;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VirtualMachine {
    pub display_name: String,
    pub path: BundlePath,
    #[serde(rename = "guestOS")]
    pub guest_os: String,
    pub vcpus: NonZeroU64,
    pub memory: NonZeroU64,
    pub secure_boot: bool,
    #[serde(default)]
    pub disks: VirtualDisks,
}

// Virtual disks

pub(crate) type VirtualDisks = BTreeMap<String, VirtualDisk>;

#[derive(Debug, Deserialize)]
pub(crate) struct VirtualDisk {
    pub path: DiskPath,
    pub size: NonZeroU64,
    #[serde(default)]
    pub bus: DiskBus,
    #[serde(default)]
    pub preallocate: bool,
    #[serde(default)]
    pub split: bool,
}

#[derive(Debug, Default, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DiskBus {
    #[default]
    Nvme,
    Sata,
}

// Paths

#[nutype(
    validate(predicate = valid_bundle_path),
    derive(Debug, Deserialize, AsRef),
)]
pub(crate) struct BundlePath(PathBuf);

fn valid_bundle_path(path: &Path) -> bool {
    path.is_absolute() && path.extension().is_some_and(|ext| ext == "vmwarevm")
}

#[nutype(
    validate(predicate = valid_disk_path),
    derive(Debug, Deserialize, AsRef),
)]
pub(crate) struct DiskPath(PathBuf);

fn valid_disk_path(path: &Path) -> bool {
    path.is_absolute() && path.extension().is_some_and(|ext| ext == "vmdk")
}
