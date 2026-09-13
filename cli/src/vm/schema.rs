use nutype::nutype;
use serde::Deserialize;
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualMachine {
    pub display_name: String,
    pub path: BundlePath,
    #[serde(rename = "guestOS")]
    pub guest_os: String,
    pub vcpus: NonZeroU64,
    pub memory: NonZeroU64,
    pub secure_boot: bool,
}

#[nutype(
    validate(predicate = valid_bundle_path),
    derive(Debug, Deserialize, AsRef),
)]
pub struct BundlePath(PathBuf);

fn valid_bundle_path(path: &Path) -> bool {
    path.is_absolute() && path.extension().is_some_and(|ext| ext == "vmwarevm")
}
