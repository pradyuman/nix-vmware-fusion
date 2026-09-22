use anyhow::Result;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::LazyLock;

pub(crate) static CONFIG: LazyLock<Config> =
    LazyLock::new(|| Config::load().expect("could not load configuration"));

#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    pub dmg: PathBuf,
    pub dict_tool: PathBuf,
    pub qemu_img: PathBuf,
    pub vdisk_manager: PathBuf,
    pub vmcli: PathBuf,
}

impl Config {
    fn load() -> Result<Self> {
        Ok(::config::Config::builder()
            .add_source(::config::Environment::with_prefix("NIX_VMWARE_FUSION"))
            .build()?
            .try_deserialize()?)
    }
}
