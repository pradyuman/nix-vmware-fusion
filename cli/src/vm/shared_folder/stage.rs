use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};

use crate::config::CONFIG;
use crate::vm::schema::SharedFolder;

use super::{Action, Plan};

pub(crate) fn stage(draft_path: &Path, plan: Plan) -> Result<()> {
    plan.actions
        .into_iter()
        .try_for_each(|action| match action {
            Action::Configure {
                label,
                name,
                folder,
            } => configure(draft_path, &label, &name, &folder),
            Action::Remove { label } => set(draft_path, &label, "SetPresent", "false"),
        })
}

fn configure(path: &Path, label: &str, name: &str, folder: &SharedFolder) -> Result<()> {
    let host_path = folder.host_path.as_ref();
    let metadata = fs::metadata(host_path)
        .with_context(|| format!("could not inspect shared folder {}", host_path.display()))?;

    ensure!(
        metadata.is_dir(),
        "shared folder path is not a directory: {}",
        host_path.display()
    );

    set(path, label, "SetPresent", "true")?;
    set(path, label, "SetGuestName", name)?;
    set(path, label, "SetHostPath", host_path)?;
    set(path, label, "SetReadAccess", "true")?;
    set(
        path,
        label,
        "SetWriteAccess",
        (!folder.read_only).to_string(),
    )?;
    set(path, label, "SetEnabled", "true")?;

    Ok(())
}

fn set(path: &Path, label: &str, command: &str, value: impl AsRef<OsStr>) -> Result<()> {
    duct::cmd!(&CONFIG.vmcli, path, "HGFS", command, label, value.as_ref())
        .run()
        .with_context(|| format!("could not run {command} for shared folder {label}"))?;

    Ok(())
}

#[cfg(all(test, feature = "vmware-tests"))]
mod tests {
    use crate::vm::schema::SharedFolderPath;
    use crate::vm::shared_folder::inspect;
    use crate::vm::test_support::create_vmx;

    use super::*;

    #[test]
    fn vmcli_configures_updates_and_removes_shared_folder() -> Result<()> {
        let (temp_dir, vmx_path) = create_vmx()?;
        let projects_path = temp_dir.path().join("projects");
        let downloads_path = temp_dir.path().join("downloads");
        let label = "sharedFolder0";
        let name = "projects";

        fs::create_dir(&projects_path)?;
        fs::create_dir(&downloads_path)?;

        // Configure a new read-write shared folder
        stage(
            &vmx_path,
            Plan {
                actions: vec![Action::Configure {
                    label: label.to_owned(),
                    name: name.to_owned(),
                    folder: SharedFolder {
                        host_path: SharedFolderPath::try_new(projects_path.clone())
                            .expect("valid shared folder path"),
                        read_only: false,
                    },
                }],
            },
        )?;

        let snapshot = inspect(&vmx_path)?;
        assert_eq!(snapshot.shared_folders.len(), 1);

        let folder = &snapshot.shared_folders[0];
        assert_eq!(folder.label, label);
        assert_eq!(folder.guest_name, name);
        assert_eq!(folder.host_path, projects_path);
        assert!(folder.present);
        assert!(folder.enabled);
        assert!(folder.read_access);
        assert!(folder.write_access);

        // Update the same shared folder in place and make it read-only
        stage(
            &vmx_path,
            Plan {
                actions: vec![Action::Configure {
                    label: label.to_owned(),
                    name: name.to_owned(),
                    folder: SharedFolder {
                        host_path: SharedFolderPath::try_new(downloads_path.clone())
                            .expect("valid shared folder path"),
                        read_only: true,
                    },
                }],
            },
        )?;

        let snapshot = inspect(&vmx_path)?;
        assert_eq!(snapshot.shared_folders.len(), 1);

        let folder = &snapshot.shared_folders[0];
        assert_eq!(folder.host_path, downloads_path);
        assert!(folder.read_access);
        assert!(!folder.write_access);

        // Remove the shared folder
        stage(
            &vmx_path,
            Plan {
                actions: vec![Action::Remove {
                    label: label.to_owned(),
                }],
            },
        )?;

        assert!(inspect(&vmx_path)?.shared_folders.is_empty());

        Ok(())
    }
}
