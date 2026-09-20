use anyhow::{Context, Result};
use atomic_write_file::AtomicWriteFile;
use std::fs;
use std::io::Write;

use super::StagedChange;

impl StagedChange {
    pub(crate) fn commit(self) -> Result<()> {
        let bundle_path = self
            .snapshot
            .target_path
            .parent()
            .context("missing VMX directory")?;

        // Create the bundle directory if needed
        fs::create_dir_all(bundle_path)?;

        // Atomically save the VMX contents
        let mut file = AtomicWriteFile::open(&self.snapshot.target_path)?;
        file.write_all(self.updated_contents.as_bytes())?;
        file.commit()?;

        Ok(())
    }
}
