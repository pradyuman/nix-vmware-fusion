use std::num::NonZeroU64;
use std::path::PathBuf;

use crate::vm::schema::DiskBus;

mod commit;

mod format;
pub(crate) use format::DiskFormat;

mod inspect;
pub(crate) use inspect::inspect;

mod plan;
pub(crate) use plan::plan;

mod stage;
pub(crate) use stage::stage;

pub(crate) const BYTES_PER_GIB: u64 = 1024_u64.pow(3);

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
    pub size: NonZeroU64,
    pub bus: DiskBus,
    pub format: DiskFormat,
    pub canonical_path: PathBuf,
    pub current_state: DiskState,
}

#[derive(Debug)]
pub(crate) struct DiskState {
    pub capacity_bytes: NonZeroU64,
    pub format: DiskFormat,
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
    Move { from: DiskLabel, to: DiskBus },
    Attach { path: PathBuf, to: DiskBus },
    Detach { label: DiskLabel },
    Expand { path: PathBuf, size: NonZeroU64 },
    Convert { path: PathBuf, format: DiskFormat },
}

// Stage

pub(crate) struct StagedChange {
    commit_actions: Vec<CommitAction>,
}

enum CommitAction {
    Expand { path: PathBuf, size: NonZeroU64 },
    Convert { path: PathBuf, format: DiskFormat },
}
