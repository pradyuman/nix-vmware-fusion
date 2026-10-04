use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::state_file::PersistentState;

use super::schema::VmnetName;

const STATE_PATH: &str = "/Library/Application Support/nix-vmware-fusion/networks.json";

pub(crate) type ManagedNetworks = BTreeMap<VmnetName, NetworkMetadata>;

#[derive(Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct State {
    #[serde(default)]
    pub networks: ManagedNetworks,
}

#[derive(Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct NetworkMetadata {}

impl PersistentState for State {
    const FORMAT_VERSION: u64 = 1;
}

impl State {
    pub(crate) fn load() -> Result<Self> {
        Self::load_from(Path::new(STATE_PATH))
    }

    pub(crate) fn save(&self) -> Result<()> {
        self.save_to(Path::new(STATE_PATH))
    }
}
