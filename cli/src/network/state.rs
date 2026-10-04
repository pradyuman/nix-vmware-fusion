use std::collections::BTreeMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::config::CONFIG;
use crate::state_file::PersistentState;

use super::schema::VmnetName;

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
        Self::load_from(&CONFIG.network_state)
    }

    pub(crate) fn save(&self) -> Result<()> {
        self.save_to(&CONFIG.network_state)
    }
}
