use std::path::PathBuf;

mod inspect;
pub(super) use inspect::inspect;

mod plan;
pub(super) use plan::plan;

mod stage;
pub(super) use stage::stage;

use crate::vm::schema::SharedFolder;

pub(super) type SharedFolderLabel = String;

// Inspect

#[derive(Debug)]
pub(super) struct Snapshot {
    pub shared_folders: Vec<ObservedSharedFolder>,
}

#[derive(Debug)]
pub(super) struct ObservedSharedFolder {
    pub label: SharedFolderLabel,
    pub guest_name: String,
    pub host_path: PathBuf,
    pub present: bool,
    pub enabled: bool,
    pub read_access: bool,
    pub write_access: bool,
}

// Plan

#[derive(Debug)]
pub(super) struct Plan {
    pub actions: Vec<Action>,
}

#[derive(Debug)]
pub(super) enum Action {
    Configure {
        label: SharedFolderLabel,
        name: String,
        folder: SharedFolder,
    },
    Remove {
        label: SharedFolderLabel,
    },
}
