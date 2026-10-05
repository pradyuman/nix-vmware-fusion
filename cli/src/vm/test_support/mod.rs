use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::config::CONFIG;

use super::vmx;

pub(super) mod assert;

pub(super) const GUEST_OS: &str = "arm-other6xlinux-64";

pub(super) fn create_vmx() -> Result<(tempfile::TempDir, PathBuf)> {
    let temp_dir = tempfile::tempdir()?;
    let vmx_path = temp_dir.path().join("test.vmx");

    vmx::create(&vmx_path, GUEST_OS)?;

    Ok((temp_dir, vmx_path))
}

pub(super) fn create_vmdk(path: &Path, size: &str) -> Result<()> {
    duct::cmd!(
        &CONFIG.vdisk_manager,
        "-c",
        "-s",
        size,
        "-a",
        "lsilogic",
        "-t",
        "0",
        "-q",
        path
    )
    .run()?;

    Ok(())
}
