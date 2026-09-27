use nutype::nutype;
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

#[nutype(derive(Clone, Debug, Deserialize, AsRef, Display, Eq, PartialEq))]
pub(crate) struct DiskLabel(String);

impl DiskLabel {
    pub(crate) fn bus(&self) -> Option<DiskBus> {
        if self.as_ref().starts_with("nvme") {
            Some(DiskBus::Nvme)
        } else if self.as_ref().starts_with("sata") {
            Some(DiskBus::Sata)
        } else {
            None
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_label_identifies_bus() {
        assert_eq!(DiskLabel::new("nvme0:0").bus(), Some(DiskBus::Nvme));
        assert_eq!(DiskLabel::new("sata0:0").bus(), Some(DiskBus::Sata));
        assert_eq!(DiskLabel::new("scsi0:0").bus(), None);
    }
}
