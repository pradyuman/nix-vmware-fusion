use std::path::PathBuf;
use std::sync::LazyLock;

use anyhow::Result;
use serde::Deserialize;

pub(crate) static CONFIG: LazyLock<Config> =
    LazyLock::new(|| Config::load().expect("could not load test configuration"));

#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    pub cli: PathBuf,
    pub dict_tool: PathBuf,
}

impl Config {
    fn load() -> Result<Self> {
        Ok(config::Config::builder()
            .add_source(config::Environment::with_prefix("NIX_VMWARE_FUSION"))
            .set_override("cli", env!("CARGO_BIN_EXE_nix-vmware-fusion"))?
            .build()?
            .try_deserialize()?)
    }
}
