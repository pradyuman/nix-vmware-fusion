use std::path::PathBuf;

use crate::vm::schema::DiskBus;

mod inspect;
pub(crate) use inspect::inspect;

mod plan;
pub(crate) use plan::plan;

mod stage;
pub(crate) use stage::stage;

pub(crate) type DiskLabel = String;

// Inspect

pub(crate) struct Snapshot {
    pub declared_disks: Vec<DeclaredDisk>,
    pub attached_disks: Vec<AttachedDisk>,
}

pub(crate) struct DeclaredDisk {
    pub path: PathBuf,
    pub bus: DiskBus,
    pub canonical_path: PathBuf,
}

pub(crate) struct AttachedDisk {
    pub label: DiskLabel,
    pub canonical_path: Option<PathBuf>,
}

// Plan

pub(crate) struct Plan {
    pub actions: Vec<Action>,
}

pub(crate) enum Action {
    Move { from: DiskLabel, to: DiskBus },
    Attach { path: PathBuf, to: DiskBus },
    Detach { label: DiskLabel },
}
