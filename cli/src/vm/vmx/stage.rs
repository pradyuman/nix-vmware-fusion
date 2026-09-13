use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::config::CONFIG;
use crate::vm::schema::VirtualMachine;

use super::{Snapshot, StagedChange};

impl StagedChange {
    pub fn is_noop(&self) -> bool {
        self.snapshot.contents.as_deref() == Some(self.contents.as_str())
    }
}

// Draft the updated contents without modifying the original VMX
pub fn stage(schema: &VirtualMachine, snapshot: Snapshot) -> Result<StagedChange> {
    let temp_dir = tempfile::tempdir()?;
    let filename = snapshot
        .target_path
        .file_name()
        .context("missing VMX filename")?;

    // Copy the existing VMX contents or create a new baseline
    let draft_path = temp_dir.path().join(filename);
    match &snapshot.contents {
        Some(contents) => fs::write(&draft_path, contents)?,
        None => create(&draft_path, &schema.guest_os)?,
    }

    // Map the requested settings to VMX keys and values
    let mut entries = vec![
        ("displayName", schema.display_name.clone()),
        ("guestOS", schema.guest_os.clone()),
        ("numvcpus", schema.vcpus.to_string()),
        ("memsize", schema.memory.to_string()),
        (
            "uefi.secureBoot.enabled",
            if schema.secure_boot { "TRUE" } else { "FALSE" }.to_owned(),
        ),
    ];

    if snapshot.contents.is_none() {
        // Fusion on Apple silicon requires UEFI; BIOS is unsupported
        // https://knowledge.broadcom.com/external/article/315602
        entries.push(("firmware", "efi".to_owned()));
    }

    // Apply every entry to the draft before returning anything to commit
    set_entries(&draft_path, entries)?;

    // Only the VMX contents are carried forward. We intentionally ignore any
    // generated disks and let the temporary directory clean them up
    let contents = fs::read_to_string(&draft_path)?;

    Ok(StagedChange { snapshot, contents })
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

fn set_entries(path: &Path, entries: Vec<(&str, String)>) -> Result<()> {
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

    fn staged_change(prev: Option<&str>, contents: &str) -> StagedChange {
        StagedChange {
            snapshot: Snapshot {
                target_path: "example.vmx".into(),
                contents: prev.map(str::to_owned),
            },
            contents: contents.to_owned(),
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
