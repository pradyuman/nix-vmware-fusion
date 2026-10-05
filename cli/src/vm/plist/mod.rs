use std::path::PathBuf;

mod commit;
mod inspect;
pub(super) use inspect::inspect;

mod plan;
pub(super) use plan::plan;

mod stage;
pub(super) use stage::stage;

// Inspect

#[derive(Debug)]
pub(super) struct Snapshot {
    pub target_path: PathBuf,
    raw_contents: Option<Vec<u8>>,
}

// Plan

pub(super) struct Plan {
    pub snapshot: Snapshot,
    action: Action,
}

enum Action {
    SetScaledHighResolution(u8),
    RemoveScaledHighResolution,
}

// Stage

pub(super) struct StagedChange {
    write: Option<StagedWrite>,
}

struct StagedWrite {
    path: PathBuf,
    contents: Vec<u8>,
}
