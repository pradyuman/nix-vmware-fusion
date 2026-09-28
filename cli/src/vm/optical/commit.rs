use std::path::Path;

use anyhow::Result;

use super::StagedChange;

impl StagedChange {
    pub(crate) fn commit(self, bundle_path: &Path) -> Result<()> {
        self.state.save(bundle_path)
    }
}
