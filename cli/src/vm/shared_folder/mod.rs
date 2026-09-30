use std::path::PathBuf;

mod inspect;
pub(crate) use inspect::inspect;

mod plan;
pub(crate) use plan::plan;

mod stage;
pub(crate) use stage::stage;

use crate::vm::schema::SharedFolder;

pub(crate) type SharedFolderLabel = String;

// Inspect

#[derive(Debug)]
pub(crate) struct Snapshot {
    pub shared_folders: Vec<ObservedSharedFolder>,
}

#[derive(Debug)]
pub(crate) struct ObservedSharedFolder {
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
pub(crate) struct Plan {
    pub actions: Vec<Action>,
}

#[derive(Debug)]
pub(crate) enum Action {
    Configure {
        label: SharedFolderLabel,
        name: String,
        folder: SharedFolder,
    },
    Remove {
        label: SharedFolderLabel,
    },
}
