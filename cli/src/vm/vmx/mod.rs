use std::path::PathBuf;

mod commit;
mod inspect;
mod stage;

pub use inspect::inspect;
pub use stage::stage;

pub struct Snapshot {
    target_path: PathBuf,
    contents: Option<String>,
}

pub struct StagedChange {
    snapshot: Snapshot,
    contents: String,
}
