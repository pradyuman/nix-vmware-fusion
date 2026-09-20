use std::path::PathBuf;

mod commit;
mod inspect;
pub(crate) use inspect::inspect;

mod plan;
pub(crate) use plan::plan;

mod stage;
pub(crate) use stage::stage;

// Inspect

#[derive(Debug)]
pub(crate) struct Snapshot {
    pub target_path: PathBuf,
    raw_contents: Option<String>,
}

// Plan

pub(crate) struct Plan {
    pub snapshot: Snapshot,
    guest_os: String,
    entries: Vec<(&'static str, String)>,
}

// Stage

pub(crate) struct StagedChange {
    pub snapshot: Snapshot,
    pub updated_contents: String,
}
