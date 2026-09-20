use std::path::PathBuf;

use crate::vm::schema::VirtualDiskBus;

mod inspect;
pub(crate) use inspect::inspect;

mod plan;
pub(crate) use plan::plan;

mod stage;
pub(crate) use stage::stage;

pub(crate) type DiskLabel = String;

// Inspect

#[derive(Debug)]
pub(crate) struct Snapshot {
    pub configured_disks: Vec<ConfiguredDisk>,
    pub attached_disks: Vec<AttachedDisk>,
}

#[derive(Debug)]
pub(crate) struct ConfiguredDisk {
    pub path: PathBuf,
    pub bus: VirtualDiskBus,
    pub canonical_path: PathBuf,
}

#[derive(Debug)]
pub(crate) struct AttachedDisk {
    pub label: DiskLabel,
    pub canonical_path: Option<PathBuf>,
}

// Plan

#[derive(Debug)]
pub(crate) struct Plan {
    pub actions: Vec<Action>,
}

#[derive(Debug)]
pub(crate) enum Action {
    Move { from: DiskLabel, to: VirtualDiskBus },
    Attach { path: PathBuf, to: VirtualDiskBus },
    Detach { label: DiskLabel },
}
