use std::path::PathBuf;

mod inspect;
pub(super) use inspect::inspect;

mod plan;
pub(super) use plan::plan;

mod stage;
pub(super) use stage::stage;

use crate::vm::schema::SharedFolder;

type SharedFolderLabel = String;

// Inspect

#[derive(Debug)]
pub(super) struct Snapshot {
    shared_folders: Vec<ObservedSharedFolder>,
}

#[derive(Debug)]
struct ObservedSharedFolder {
    label: SharedFolderLabel,
    guest_name: String,
    host_path: PathBuf,
    present: bool,
    enabled: bool,
    read_access: bool,
    write_access: bool,
}

// Plan

#[derive(Debug)]
pub(super) struct Plan {
    actions: Vec<Action>,
}

#[derive(Debug)]
enum Action {
    Configure {
        label: SharedFolderLabel,
        name: String,
        folder: SharedFolder,
    },
    Remove {
        label: SharedFolderLabel,
    },
}
