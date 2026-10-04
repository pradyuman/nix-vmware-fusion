use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::state_file::PersistentState;

use super::optical::OpticalDriveLabel;

const FILENAME: &str = ".nix-vmware-fusion.json";

pub(super) type OpticalDriveBindings = BTreeMap<String, OpticalDriveBinding>;

#[derive(Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct State {
    #[serde(default)]
    pub optical_drives: OpticalDriveBindings,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct OpticalDriveBinding {
    pub label: OpticalDriveLabel,
}

impl PersistentState for State {
    const FORMAT_VERSION: u64 = 1;
}

impl State {
    pub(super) fn load(bundle_path: &Path) -> Result<Self> {
        Self::load_from(&bundle_path.join(FILENAME))
    }

    pub(super) fn save(&self, bundle_path: &Path) -> Result<()> {
        self.save_to(&bundle_path.join(FILENAME))
    }
}
