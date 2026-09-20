use anyhow::{Context, Result};
use atomic_write_file::AtomicWriteFile;
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::config::CONFIG;

use super::StagedChange;

impl StagedChange {
    pub(crate) fn commit(self) -> Result<()> {
        let bundle_path = self
            .snapshot
            .target_path
            .parent()
            .context("missing VMX directory")?;

        // Check that the virtual machine is stopped before writing changes
        ensure_stopped(bundle_path, &self.snapshot.target_path)?;

        // Create the bundle directory if needed
        fs::create_dir_all(bundle_path)?;

        // Atomically save the VMX contents
        let mut file = AtomicWriteFile::open(&self.snapshot.target_path)?;
        file.write_all(self.updated_contents.as_bytes())?;
        file.commit()?;

        Ok(())
    }
}

fn ensure_stopped(bundle_path: &Path, vmx_path: &Path) -> Result<()> {
    if !vmx_path.try_exists()? {
        return Ok(());
    }

    let power = duct::cmd!(&CONFIG.vmcli, vmx_path, "power", "query")
        .read()
        .context("could not query virtual machine power state")?;

    let state = power
        .lines()
        .find_map(|line| line.strip_prefix("PowerState:"))
        .map(str::trim)
        .context("vmcli did not report a power state")?;

    anyhow::ensure!(
        state == "off",
        "{} has power state {state}; fully shut it down before applying changes",
        bundle_path.display()
    );

    Ok(())
}
