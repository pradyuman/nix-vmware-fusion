use std::fs;
use std::num::NonZeroU64;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::config::CONFIG;

use super::{CommitAction, DiskFormat, StagedChange};

impl StagedChange {
    pub(crate) fn is_noop(&self) -> bool {
        self.commit_actions.is_empty()
    }

    pub(crate) fn commit(self) -> Result<()> {
        self.commit_actions.into_iter().try_for_each(|action| {
            match action {
                CommitAction::Expand { path, size } => expand(&path, size)?,
                CommitAction::Convert { path, format } => convert(&path, format)?,
            }

            Ok(())
        })
    }
}

fn expand(path: &Path, size: NonZeroU64) -> Result<()> {
    duct::cmd!(&CONFIG.vdisk_manager, "-x", format!("{size}GB"), "-q", path)
        .run()
        .with_context(|| format!("could not expand disk {} to {size} GiB", path.display()))?;

    Ok(())
}

fn convert(path: &Path, format: DiskFormat) -> Result<()> {
    let directory = path.parent().context("missing disk directory")?;
    let filename = path.file_name().context("missing disk filename")?;

    // Keep the conversion transaction beside the disk so its moves stay on one filesystem
    let temp_dir = tempfile::Builder::new()
        .prefix(".nix-vmware-fusion-vmdk-convert-")
        .tempdir_in(directory)?;

    // Separate directories prevent collisions with user-selected VMDK filenames
    let converted_directory = temp_dir.path().join("converted");
    let backup_directory = temp_dir.path().join("backup");

    fs::create_dir(&converted_directory)?;
    fs::create_dir(&backup_directory)?;

    let converted_path = converted_directory.join(filename);
    let backup_path = backup_directory.join(filename);

    // vmware-vdiskmanager writes conversions to a separate destination
    create_converted_disk(path, &converted_path, format)?;

    // Move the original to the backup location after conversion succeeds
    if let Err(backup_error) = move_disk(path, &backup_path) {
        // Keep the converted disk and any files that may have moved before the failure
        let recovery_path = temp_dir.keep();
        bail!(
            concat!(
                "could not move original disk to backup location\n",
                "recovery directory: {}\n",
                "original path: {}\n",
                "backup path: {}\n",
                "converted disk: {}\n",
                "move error: {:#}",
            ),
            recovery_path.display(),
            path.display(),
            backup_path.display(),
            converted_path.display(),
            backup_error
        );
    }

    // Replace the original path, restoring the backup if replacement fails
    if let Err(replace_error) = move_disk(&converted_path, path) {
        if let Err(restore_error) = move_disk(&backup_path, path) {
            // Keep both disks available for manual recovery if rollback also fails
            let recovery_path = temp_dir.keep();
            bail!(
                concat!(
                    "could not install converted disk and rollback failed\n",
                    "recovery directory: {}\n",
                    "original disk: {}\n",
                    "converted disk: {}\n",
                    "install error: {:#}\n",
                    "rollback error: {:#}",
                ),
                recovery_path.display(),
                backup_path.display(),
                converted_path.display(),
                replace_error,
                restore_error
            );
        }

        return Err(replace_error)
            .context("could not install converted disk; restored original disk");
    }

    // Replacement succeeded, so the backup is no longer needed
    temp_dir
        .close()
        .context("could not remove disk conversion backup")
}

fn create_converted_disk(from: &Path, to: &Path, format: DiskFormat) -> Result<()> {
    duct::cmd!(
        &CONFIG.vdisk_manager,
        "-r",
        from,
        "-t",
        format.vdisk_type(),
        "-q",
        to
    )
    .run()
    .with_context(|| format!("could not convert disk {} to {format}", from.display()))?;

    Ok(())
}

fn move_disk(from: &Path, to: &Path) -> Result<()> {
    duct::cmd!(&CONFIG.vdisk_manager, "-n", from, "-q", to)
        .run()
        .with_context(|| {
            format!(
                "could not move disk from {} to {}",
                from.display(),
                to.display()
            )
        })?;

    Ok(())
}

#[cfg(all(test, feature = "vmware-tests"))]
mod tests {
    use crate::vm::disks::inspect::read_format;
    use crate::vm::test_support::create_partitioned_vmdk;

    use super::*;

    #[test]
    fn vmware_tools_convert_disk_formats() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let disk_path = temp_dir.path().join("managed.vmdk");
        let formats = [
            DiskFormat::SplitSparse,
            DiskFormat::Preallocated,
            DiskFormat::SplitPreallocated,
            DiskFormat::Sparse,
        ];

        create_partitioned_vmdk(&disk_path, "10MiB")?;
        assert_eq!(read_format(&disk_path)?, DiskFormat::Sparse);

        formats.into_iter().try_for_each(|format| -> Result<()> {
            StagedChange {
                commit_actions: vec![CommitAction::Convert {
                    path: disk_path.clone(),
                    format,
                }],
            }
            .commit()?;

            assert_eq!(read_format(&disk_path)?, format);

            Ok(())
        })
    }
}
