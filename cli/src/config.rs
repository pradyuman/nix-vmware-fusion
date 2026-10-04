use anyhow::Result;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::LazyLock;

const DEFAULT_NETWORK_STATE: &str = "/Library/Application Support/nix-vmware-fusion/networks.json";

pub(crate) static CONFIG: LazyLock<Config> =
    LazyLock::new(|| Config::load().expect("could not load configuration"));

#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    pub dmg: PathBuf,
    pub dict_tool: PathBuf,
    pub qemu_img: PathBuf,
    pub vdisk_manager: PathBuf,
    pub vmcli: PathBuf,
    pub vmnet_cfgcli: PathBuf,
    pub vmnet_cli: PathBuf,
    pub network_state: PathBuf,
}

impl Config {
    fn load() -> Result<Self> {
        Ok(::config::Config::builder()
            .set_default("network_state", DEFAULT_NETWORK_STATE)?
            .add_source(::config::Environment::with_prefix("NIX_VMWARE_FUSION"))
            .build()?
            .try_deserialize()?)
    }
}
