use anyhow::{Context, Result};

use crate::config::CONFIG;

use super::{CommitAction, StagedChange};

impl StagedChange {
    pub(crate) fn is_noop(&self) -> bool {
        self.commit_actions.is_empty()
    }

    pub(crate) fn commit(self) -> Result<()> {
        self.commit_actions.into_iter().try_for_each(|action| {
            match action {
                CommitAction::Expand { path, size } => {
                    duct::cmd!(
                        &CONFIG.vdisk_manager,
                        "-x",
                        format!("{size}GB"),
                        "-q",
                        &path
                    )
                    .run()
                    .with_context(|| {
                        format!("could not expand disk {} to {size} GiB", path.display())
                    })?;
                }
            }

            Ok(())
        })
    }
}
