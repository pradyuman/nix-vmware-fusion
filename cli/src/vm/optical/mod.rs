use std::path::PathBuf;

use crate::vm::schema::OpticalDrive;

use super::state::State;

mod commit;

mod inspect;
pub(crate) use inspect::inspect;

mod plan;
pub(crate) use plan::plan;

mod stage;
pub(crate) use stage::stage;

pub(crate) type OpticalDriveLabel = String;

// Inspect

#[derive(Debug)]
pub(crate) struct Snapshot {
    pub state: State,
    pub optical_attachments: Vec<OpticalAttachment>,
}

#[derive(Debug)]
pub(crate) struct OpticalAttachment {
    pub label: OpticalDriveLabel,
    pub backing_type: Option<String>,
    pub backing_path: Option<PathBuf>,
    pub client_device: bool,
    pub start_connected: bool,
}

// Plan

#[derive(Debug)]
pub(crate) struct Plan {
    pub state: State,
    pub actions: Vec<Action>,
}

#[derive(Debug)]
pub(crate) enum Action {
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

pub(crate) struct StagedChange {
    state: State,
}
