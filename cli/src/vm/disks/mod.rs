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
    pub inspected_disks: Vec<InspectedDisk>,
    pub attached_disks: Vec<AttachedDisk>,
}

#[derive(Debug)]
pub(crate) struct ConfiguredDisk {
    pub path: PathBuf,
    pub size: NonZeroU64,
    pub bus: DiskBus,
    pub format: DiskFormat,
}

#[derive(Debug)]
pub(crate) struct InspectedDisk {
    pub configured: ConfiguredDisk,
    pub identity_path: PathBuf,
    pub current_state: Option<DiskState>,
}

#[derive(Debug, PartialEq)]
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
    Create {
        path: PathBuf,
        size: NonZeroU64,
        format: DiskFormat,
    },
    Move {
        from: DiskLabel,
        to: DiskBus,
    },
    Attach {
        path: PathBuf,
        to: DiskBus,
    },
    Detach {
        label: DiskLabel,
    },
    Expand {
        path: PathBuf,
        size: NonZeroU64,
    },
    Convert {
        path: PathBuf,
        format: DiskFormat,
    },
}

// Stage

pub(crate) struct StagedChange {
    commit_actions: Vec<CommitAction>,
}

#[derive(Debug, PartialEq)]
enum CommitAction {
    Create {
        path: PathBuf,
        size: NonZeroU64,
        format: DiskFormat,
    },
    Expand {
        path: PathBuf,
        size: NonZeroU64,
    },
    Convert {
        path: PathBuf,
        format: DiskFormat,
    },
}
