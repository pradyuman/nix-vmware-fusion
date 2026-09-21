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
                target_path: "test.vmx".into(),
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

    #[cfg(feature = "vmware-contract-tests")]
    mod vmware {
        use std::path::{Path, PathBuf};

        use super::*;

        const GUEST_OS: &str = "arm-other6xlinux-64";

        fn create_vmx() -> Result<(tempfile::TempDir, PathBuf)> {
            let temp_dir = tempfile::tempdir()?;
            let vmx_path = temp_dir.path().join("test.vmx");

            create(&vmx_path, GUEST_OS)?;

            Ok((temp_dir, vmx_path))
        }

        fn query_entry(path: &Path, key: &str) -> Result<String> {
            Ok(duct::cmd!(&CONFIG.dict_tool, "-q", "query", path, key).read()?)
        }

        fn assert_entry(path: &Path, key: &str, value: &str) -> Result<()> {
            assert_eq!(query_entry(path, key)?, format!(r#"{key} = "{value}""#));

            Ok(())
        }

        #[test]
        fn vmcli_creates_vmx() -> Result<()> {
            let (_temp_dir, vmx_path) = create_vmx()?;

            assert_entry(&vmx_path, "guestOS", GUEST_OS)?;

            Ok(())
        }

        #[test]
        fn dict_tool_sets_and_updates_vmx_entries() -> Result<()> {
            let (_temp_dir, vmx_path) = create_vmx()?;

            // Test entries
            let display_name = "Test VM";
            let updated_display_name = "Updated Test VM";
            let vcpus = "4";

            // Seed entries
            set_entries(
                &vmx_path,
                &[
                    ("displayName", display_name.to_owned()),
                    ("numvcpus", vcpus.to_owned()),
                ],
            )?;

            // Set displayName again to verify that dictTool replaces the existing entry
            set_entries(
                &vmx_path,
                &[("displayName", updated_display_name.to_owned())],
            )?;

            assert_entry(&vmx_path, "displayName", updated_display_name)?;
            assert_entry(&vmx_path, "numvcpus", vcpus)?;

            Ok(())
        }

        #[test]
        fn vmcli_reports_new_vmx_as_stopped() -> Result<()> {
            let (_temp_dir, vmx_path) = create_vmx()?;

            crate::vm::ensure_stopped(&vmx_path)?;

            Ok(())
        }
    }
}
