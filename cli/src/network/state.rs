use std::collections::BTreeMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::CONFIG;
use crate::state_file::PersistentState;

use super::schema::VmnetName;

pub(super) type ManagedNetworks = BTreeMap<VmnetName, NetworkMetadata>;

#[derive(Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct State {
    #[serde(default)]
    pub networks: ManagedNetworks,
}

#[derive(Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct NetworkMetadata {}

impl PersistentState for State {
    const FORMAT_VERSION: u64 = 1;
}

impl State {
    pub(super) fn load() -> Result<Self> {
        Self::load_from(&CONFIG.network_state)
    }

    pub(super) fn save(&self) -> Result<()> {
        self.save_to(&CONFIG.network_state)
    }
}
