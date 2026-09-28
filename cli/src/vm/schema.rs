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
    pub network_adapters: NetworkAdapters,
    #[serde(default)]
    pub disks: VirtualDisks,
    #[serde(default)]
    pub optical_drives: OpticalDrives,
}

// Network adapters

pub(crate) type NetworkAdapters = BTreeMap<String, NetworkAdapter>;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NetworkAdapter {
    #[serde(default)]
    pub mode: NetworkMode,
    pub vmnet: Option<String>,
    #[serde(default)]
    pub model: NetworkAdapterModel,
    #[serde(default = "default_true")]
    pub start_connected: bool,
}

#[derive(Debug, Default, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum NetworkMode {
    #[default]
    Nat,
    Bridged,
    HostOnly,
    Custom,
}

#[derive(Debug, Default, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum NetworkAdapterModel {
    #[default]
    Vmxnet3,
    E1000e,
    E1000,
}

fn default_true() -> bool {
    true
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

// Optical drives

pub(crate) type OpticalDrives = BTreeMap<String, OpticalDrive>;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpticalDrive {
    pub source: OpticalDriveSource,
    #[serde(default = "default_true")]
    pub start_connected: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub(crate) enum OpticalDriveSource {
    Image { path: OpticalImagePath },
}

// Paths

#[nutype(
    validate(predicate = |path| valid_path(path, "vmwarevm")),
    derive(Debug, Deserialize, AsRef),
)]
pub(crate) struct BundlePath(PathBuf);

#[nutype(
    validate(predicate = |path| valid_path(path, "vmdk")),
    derive(Debug, Deserialize, AsRef),
)]
pub(crate) struct DiskPath(PathBuf);

#[nutype(
    validate(predicate = |path| valid_path(path, "iso")),
    derive(Clone, Debug, Deserialize, AsRef),
)]
pub(crate) struct OpticalImagePath(PathBuf);

fn valid_path(path: &Path, extension: &str) -> bool {
    path.is_absolute() && path.extension().is_some_and(|actual| actual == extension)
}
