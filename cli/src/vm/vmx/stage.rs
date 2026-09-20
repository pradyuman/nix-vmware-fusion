use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::config::CONFIG;

use super::{Plan, StagedChange};

impl StagedChange {
    pub(crate) fn is_noop(&self) -> bool {
        self.snapshot.raw_contents.as_deref() == Some(self.updated_contents.as_str())
    }
}

// Draft the updated contents without modifying the original VMX
pub(crate) fn stage(draft_path: &Path, plan: &Plan) -> Result<()> {
    // Copy the existing VMX contents or create a new baseline
    match &plan.snapshot.raw_contents {
        Some(contents) => fs::write(draft_path, contents)?,
        None => create(draft_path, &plan.guest_os)?,
    }

    // Apply every entry to the draft before returning anything to commit
    set_entries(draft_path, &plan.entries)?;

    Ok(())
}

fn create(path: &Path, guest_os: &str) -> Result<()> {
    let name = path.file_stem().context("missing virtual machine name")?;
    let directory_path = path.parent().context("missing VMX directory")?;

    duct::cmd!(
        &CONFIG.vmcli,
        "vm",
        "create",
        "-n",
        name,
        "-d",
        directory_path,
        "-c",
        guest_os
    )
    .run()?;

    Ok(())
}

fn set_entries(path: &Path, entries: &[(&str, String)]) -> Result<()> {
    for (key, value) in entries {
        duct::cmd!(
            &CONFIG.dict_tool,
            "-q",
            "set",
            path,
            format!("{key}={value}")
        )
        .run()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::vmx::Snapshot;

    fn staged_change(prev: Option<&str>, updated_contents: &str) -> StagedChange {
        StagedChange {
            snapshot: Snapshot {
                target_path: "example.vmx".into(),
                raw_contents: prev.map(str::to_owned),
            },
            updated_contents: updated_contents.to_owned(),
        }
    }

    #[test]
    fn identical_contents_are_a_noop() {
        let staged = staged_change(Some("numvcpus = \"2\"\n"), "numvcpus = \"2\"\n");

        assert!(staged.is_noop());
    }

    #[test]
    fn changed_contents_are_not_a_noop() {
        let staged = staged_change(Some("numvcpus = \"2\"\n"), "numvcpus = \"4\"\n");

        assert!(!staged.is_noop());
    }

    #[test]
    fn missing_file_is_not_a_noop_even_with_empty_contents() {
        let staged = staged_change(None, "");

        assert!(!staged.is_noop());
    }
}
