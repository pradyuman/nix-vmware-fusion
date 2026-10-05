use anyhow::Result;
use std::fs;
use std::path::Path;

use super::Snapshot;

pub(crate) fn inspect(vmx_path: &Path) -> Result<Snapshot> {
    let target_path = vmx_path.with_extension("plist");
    let raw_contents = target_path
        .try_exists()?
        .then(|| fs::read(&target_path))
        .transpose()?;

    Ok(Snapshot {
        target_path,
        raw_contents,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_uses_the_vmx_filename() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let vmx_path = temp_dir.path().join("custom.vmx");

        let snapshot = inspect(&vmx_path)?;

        assert_eq!(snapshot.target_path, temp_dir.path().join("custom.plist"));
        assert_eq!(snapshot.raw_contents, None);

        Ok(())
    }
}
