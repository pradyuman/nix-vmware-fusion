use std::fs;
use std::io::Write;

use anyhow::{Context, Result};
use atomic_write_file::AtomicWriteFile;

use super::StagedChange;

impl StagedChange {
    pub(crate) fn is_noop(&self) -> bool {
        self.snapshot.raw_contents.as_deref() == Some(self.updated_contents.as_str())
    }

    pub(crate) fn commit(self) -> Result<()> {
        if self.is_noop() {
            return Ok(());
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::vmx::Snapshot;

    fn staged_change(prev: Option<&str>, updated_contents: &str) -> StagedChange {
        StagedChange {
            snapshot: Snapshot {
                target_path: "test.vmx".into(),
                raw_contents: prev.map(str::to_owned),
            },
            updated_contents: updated_contents.to_owned(),
        }
    }

    #[test]
    fn missing_file_is_not_a_noop_even_with_empty_contents() {
        let staged = staged_change(None, "");

        assert!(!staged.is_noop());
    }
}
