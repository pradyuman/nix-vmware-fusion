use anyhow::{Context, Result};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::Snapshot;

pub(crate) fn inspect(bundle_path: &Path) -> Result<Snapshot> {
    let target_path = resolve_path(bundle_path)?;

    let raw_contents = target_path
        .try_exists()?
        .then(|| fs::read_to_string(&target_path))
        .transpose()?;

    Ok(Snapshot {
        target_path,
        raw_contents,
    })
}

fn resolve_path(bundle_path: &Path) -> Result<PathBuf> {
    let dir_entries = bundle_path
        .try_exists()?
        .then(|| fs::read_dir(bundle_path)?.collect::<io::Result<Vec<_>>>())
        .transpose()?
        .unwrap_or_default();

    let vmx_files = dir_entries
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "vmx"))
        .collect::<Vec<_>>();

    let target_path = match vmx_files.as_slice() {
        // No VMX: choose a filename based on the bundle name
        [] => {
            let mut filename = bundle_path
                .file_stem()
                .context("missing virtual machine name")?
                .to_os_string();
            filename.push(".vmx");
            bundle_path.join(filename)
        }
        // One VMX: use its existing path
        [vmx_path] => vmx_path.clone(),
        // Multiple VMX files: refuse to guess which one to manage
        _ => anyhow::bail!("multiple VMX files found in {}", bundle_path.display()),
    };

    Ok(target_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_bundle_is_not_created() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let bundle_path = temp_dir.path().join("test.vmwarevm");

        let snapshot = inspect(&bundle_path)?;

        assert_eq!(snapshot.raw_contents, None);
        assert!(!bundle_path.try_exists()?);

        Ok(())
    }

    #[test]
    fn empty_bundle_uses_bundle_name_for_vmx() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let bundle_path = temp_dir.path().join("test.vmwarevm");

        fs::create_dir(&bundle_path)?;

        let snapshot = inspect(&bundle_path)?;

        assert_eq!(snapshot.target_path, bundle_path.join("test.vmx"));
        assert_eq!(snapshot.raw_contents, None);
        assert!(!snapshot.target_path.try_exists()?);

        Ok(())
    }

    #[test]
    fn existing_vmx_path_and_contents_are_returned() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let target_path = temp_dir.path().join("custom.vmx");
        let contents = "displayName = \"Test VM\"\n";

        fs::write(&target_path, contents)?;

        let snapshot = inspect(temp_dir.path())?;

        assert_eq!(snapshot.target_path, target_path);
        assert_eq!(snapshot.raw_contents.as_deref(), Some(contents));

        Ok(())
    }

    #[test]
    fn disk_and_backup_files_are_not_vmx_candidates() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let target_path = temp_dir.path().join("test.vmx");

        fs::write(&target_path, "")?;
        fs::write(temp_dir.path().join("test.vmdk"), "")?;
        fs::write(temp_dir.path().join("test.vmx.bak"), "")?;

        let snapshot = inspect(temp_dir.path())?;

        assert_eq!(snapshot.target_path, target_path);

        Ok(())
    }

    #[test]
    fn bundles_with_multiple_vmx_files_are_rejected() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;

        fs::write(temp_dir.path().join("first.vmx"), "")?;
        fs::write(temp_dir.path().join("second.vmx"), "")?;

        let error = inspect(temp_dir.path()).expect_err("multiple VMX files should fail");

        assert!(error.to_string().contains("multiple VMX files found"));

        Ok(())
    }
}
