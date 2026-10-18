use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};

use super::{Action, Plan, SharedFolderLabel};
use crate::vm::schema::SharedFolder;
use crate::vm::vmcli;

pub(crate) fn stage(draft_path: &Path, plan: Plan) -> Result<()> {
    plan.actions
        .into_iter()
        .try_for_each(|action| match action {
            Action::Configure {
                label,
                name,
                folder,
            } => configure(draft_path, &label, &name, &folder)
                .with_context(|| format!("could not configure shared folder {label}")),
            Action::Remove { label } => {
                vmcli::run(draft_path, "HGFS", "SetPresent", &label, "false")
                    .with_context(|| format!("could not remove shared folder {label}"))
            }
        })
}

fn configure(
    path: &Path,
    label: &SharedFolderLabel,
    name: &str,
    folder: &SharedFolder,
) -> Result<()> {
    let host_path = folder.host_path.as_ref();
    let metadata = fs::metadata(host_path)
        .with_context(|| format!("could not inspect shared folder {}", host_path.display()))?;

    ensure!(
        metadata.is_dir(),
        "shared folder path is not a directory: {}",
        host_path.display()
    );

    vmcli::run(path, "HGFS", "SetPresent", label, "true")?;
    vmcli::run(path, "HGFS", "SetGuestName", label, name)?;
    vmcli::run(path, "HGFS", "SetHostPath", label, host_path)?;
    vmcli::run(path, "HGFS", "SetReadAccess", label, "true")?;
    vmcli::run(
        path,
        "HGFS",
        "SetWriteAccess",
        label,
        (!folder.read_only).to_string(),
    )?;
    vmcli::run(path, "HGFS", "SetEnabled", label, "true")?;

    Ok(())
}

#[cfg(all(test, feature = "vmware-tests"))]
mod tests {
    use super::*;
    use crate::vm::schema::SharedFolderPath;
    use crate::vm::shared_folder::inspect;
    use crate::vm::test_support::create_vmx;

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
                    label: SharedFolderLabel::new(label),
                    name: name.to_owned(),
                    folder: SharedFolder {
                        host_path: SharedFolderPath::try_new(projects_path.clone()).unwrap(),
                        read_only: false,
                    },
                }],
            },
        )?;

        let snapshot = inspect(&vmx_path)?;
        assert_eq!(snapshot.shared_folders.len(), 1);

        let folder = &snapshot.shared_folders[0];
        assert_eq!(folder.label.as_ref(), label);
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
                    label: SharedFolderLabel::new(label),
                    name: name.to_owned(),
                    folder: SharedFolder {
                        host_path: SharedFolderPath::try_new(downloads_path.clone()).unwrap(),
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
                    label: SharedFolderLabel::new(label),
                }],
            },
        )?;

        assert!(inspect(&vmx_path)?.shared_folders.is_empty());

        Ok(())
    }
}
