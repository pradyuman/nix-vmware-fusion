use anyhow::Result;
use atomic_write_file::AtomicWriteFile;
use std::io::Write;

use super::StagedChange;

impl StagedChange {
    pub(crate) fn is_noop(&self) -> bool {
        self.write.is_none()
    }

    pub(crate) fn commit(self) -> Result<()> {
        if let Some(write) = self.write {
            let mut file = AtomicWriteFile::open(write.path)?;
            file.write_all(&write.contents)?;
            file.commit()?;
        }

        Ok(())
    }
}
