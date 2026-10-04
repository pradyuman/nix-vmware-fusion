use std::path::PathBuf;

use crate::vm::schema::OpticalDrive;

use super::state::State;

mod commit;

mod inspect;
pub(super) use inspect::inspect;

mod plan;
pub(super) use plan::plan;

mod stage;
pub(super) use stage::stage;

pub(super) type OpticalDriveLabel = String;

// Inspect

#[derive(Debug)]
pub(super) struct Snapshot {
    pub state: State,
    pub optical_attachments: Vec<OpticalAttachment>,
}

#[derive(Debug)]
pub(super) struct OpticalAttachment {
    pub label: OpticalDriveLabel,
    pub backing_type: Option<String>,
    pub backing_path: Option<PathBuf>,
    pub client_device: bool,
    pub start_connected: bool,
}

// Plan

#[derive(Debug)]
pub(super) struct Plan {
    pub state: State,
    pub actions: Vec<Action>,
}

#[derive(Debug)]
pub(super) enum Action {
    Configure {
        name: String,
        label: Option<OpticalDriveLabel>,
        drive: OpticalDrive,
    },
    Detach {
        label: OpticalDriveLabel,
    },
}

// Stage

pub(super) struct StagedChange {
    state: State,
}
