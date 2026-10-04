use std::collections::HashSet;

use anyhow::{Result, ensure};

use crate::vm::schema::{SharedFolder, SharedFolders};

use super::{Action, ObservedSharedFolder, Plan, SharedFolderLabel, Snapshot};

struct PlanningState {
    unclaimed_folders: Vec<ObservedSharedFolder>,
    occupied_labels: HashSet<SharedFolderLabel>,
    pending_folders: Vec<(String, SharedFolder)>,
    configure_actions: Vec<Action>,
}

pub(crate) fn plan(configured: &SharedFolders, snapshot: Snapshot) -> Result<Plan> {
    let state = configured
        .iter()
        .fold(PlanningState::new(snapshot)?, |state, (name, folder)| {
            state.match_folder(name, folder)
        });

    Ok(state.assign_pending_folders().into_plan())
}

impl PlanningState {
    fn new(snapshot: Snapshot) -> Result<Self> {
        let guest_names = snapshot
            .shared_folders
            .iter()
            .map(|folder| folder.guest_name.as_str())
            .collect::<HashSet<_>>();

        ensure!(
            guest_names.len() == snapshot.shared_folders.len(),
            "multiple shared folders use the same guest name"
        );

        let occupied_labels = snapshot
            .shared_folders
            .iter()
            .map(|folder| folder.label.clone())
            .collect();

        Ok(Self {
            unclaimed_folders: snapshot.shared_folders,
            occupied_labels,
            pending_folders: Vec::new(),
            configure_actions: Vec::new(),
        })
    }

    fn match_folder(mut self, name: &str, folder: &SharedFolder) -> Self {
        let observed = self
            .unclaimed_folders
            .iter()
            .position(|observed| observed.guest_name == name)
            .map(|index| self.unclaimed_folders.remove(index));

        match observed {
            Some(observed) => {
                if !matches_configuration(&observed, folder) {
                    self.configure_actions.push(Action::Configure {
                        label: observed.label,
                        name: name.to_owned(),
                        folder: folder.clone(),
                    });
                }
            }
            None => self.pending_folders.push((name.to_owned(), folder.clone())),
        }

        self
    }

    fn assign_pending_folders(mut self) -> Self {
        let pending_folders = std::mem::take(&mut self.pending_folders);

        pending_folders
            .into_iter()
            .fold(self, |mut state, (name, folder)| {
                // Reuse an unclaimed shared folder before allocating a new VMware label.
                let label = if state.unclaimed_folders.is_empty() {
                    let label = first_free_label(&state.occupied_labels);
                    state.occupied_labels.insert(label.clone());
                    label
                } else {
                    state.unclaimed_folders.remove(0).label
                };

                state.configure_actions.push(Action::Configure {
                    label,
                    name,
                    folder,
                });

                state
            })
    }

    fn into_plan(self) -> Plan {
        let remove_actions = self
            .unclaimed_folders
            .into_iter()
            .map(|folder| Action::Remove {
                label: folder.label,
            });

        Plan {
            actions: remove_actions.chain(self.configure_actions).collect(),
        }
    }
}

fn first_free_label(occupied_labels: &HashSet<SharedFolderLabel>) -> SharedFolderLabel {
    (0..)
        .map(|index| format!("sharedFolder{index}"))
        .find(|label| !occupied_labels.contains(label))
        .expect("finite shared folders leave a free label")
}

fn matches_configuration(observed: &ObservedSharedFolder, configured: &SharedFolder) -> bool {
    observed.host_path.as_path() == configured.host_path.as_ref()
        && observed.present
        && observed.enabled
        && observed.read_access
        && observed.write_access == !configured.read_only
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::vm::schema::SharedFolderPath;

    use super::*;

    fn configured_folders(name: &str, path: &str, read_only: bool) -> SharedFolders {
        SharedFolders::from([(
            name.to_owned(),
            SharedFolder {
                host_path: SharedFolderPath::try_new(PathBuf::from(path))
                    .expect("valid shared folder path"),
                read_only,
            },
        )])
    }

    fn observed_folder(label: &str, name: &str, path: &str) -> ObservedSharedFolder {
        ObservedSharedFolder {
            label: label.to_owned(),
            guest_name: name.to_owned(),
            host_path: path.into(),
            present: true,
            enabled: true,
            read_access: true,
            write_access: true,
        }
    }

    #[test]
    fn matching_shared_folder_is_unchanged() -> Result<()> {
        let path = "/Users/asuna/projects";
        let configured = configured_folders("projects", path, false);
        let snapshot = Snapshot {
            shared_folders: vec![observed_folder("sharedFolder0", "projects", path)],
        };

        assert!(plan(&configured, snapshot)?.actions.is_empty());

        Ok(())
    }

    #[test]
    fn changed_shared_folder_is_configured() -> Result<()> {
        let path = "/Users/asuna/projects";
        let configured = configured_folders("projects", path, true);
        let snapshot = Snapshot {
            shared_folders: vec![observed_folder("sharedFolder0", "projects", path)],
        };

        let plan = plan(&configured, snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Configure {
                label,
                name,
                folder,
            }] if label == "sharedFolder0"
                && name == "projects"
                && folder.host_path.as_ref() == std::path::Path::new(path)
                && folder.read_only
        ));

        Ok(())
    }

    #[test]
    fn missing_shared_folder_is_configured() -> Result<()> {
        let configured = configured_folders("projects", "/Users/asuna/projects", false);
        let snapshot = Snapshot {
            shared_folders: Vec::new(),
        };

        let plan = plan(&configured, snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Configure { label, name, .. }]
                if label == "sharedFolder0" && name == "projects"
        ));

        Ok(())
    }

    #[test]
    fn existing_shared_folder_is_reused() -> Result<()> {
        let configured = configured_folders("projects", "/Users/asuna/projects", false);
        let snapshot = Snapshot {
            shared_folders: vec![observed_folder(
                "sharedFolder0",
                "downloads",
                "/Users/asuna/Downloads",
            )],
        };

        let plan = plan(&configured, snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Configure { label, name, .. }]
                if label == "sharedFolder0" && name == "projects"
        ));

        Ok(())
    }

    #[test]
    fn undeclared_shared_folder_is_removed() -> Result<()> {
        let snapshot = Snapshot {
            shared_folders: vec![observed_folder(
                "sharedFolder0",
                "projects",
                "/Users/asuna/projects",
            )],
        };

        let plan = plan(&SharedFolders::new(), snapshot)?;

        assert!(matches!(
            plan.actions.as_slice(),
            [Action::Remove { label }] if label == "sharedFolder0"
        ));

        Ok(())
    }

    #[test]
    fn duplicate_guest_names_are_rejected() {
        let snapshot = Snapshot {
            shared_folders: vec![
                observed_folder("sharedFolder0", "projects", "/Users/asuna/projects"),
                observed_folder("sharedFolder1", "projects", "/Users/asuna/other-projects"),
            ],
        };

        let error = plan(&SharedFolders::new(), snapshot)
            .expect_err("duplicate guest names should be rejected");

        assert!(error.to_string().contains("multiple shared folders"));
    }
}
