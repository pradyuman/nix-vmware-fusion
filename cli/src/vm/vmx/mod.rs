use std::path::PathBuf;

mod commit;
mod inspect;
pub(super) use inspect::inspect;

mod plan;
pub(super) use plan::plan;

mod stage;
pub(super) use stage::stage;

#[cfg(all(test, feature = "vmware-tests"))]
pub(super) use stage::create;

// Inspect

#[derive(Debug)]
pub(super) struct Snapshot {
    pub target_path: PathBuf,
    raw_contents: Option<String>,
}

// Plan

pub(super) struct Plan {
    pub snapshot: Snapshot,
    guest_os: String,
    actions: Vec<Action>,
}

#[derive(Debug)]
enum Action {
    Set(&'static str, String),
    Remove(&'static str),
}

// Stage

pub(super) struct StagedChange {
    pub snapshot: Snapshot,
    pub updated_contents: String,
}
