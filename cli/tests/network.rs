#![cfg(feature = "vmware-privileged-tests")]

use std::collections::BTreeMap;
use std::fs;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};

use support::config;

mod support;

config! {
    cli,
    vmnet_cfgcli,
    vmnet_cli,
}

const NETWORKING_PATH: &str = "/Library/Preferences/VMware Fusion/networking";
const PREFERENCES_PATH: &str = "/Library/Preferences/VMware Fusion";

// Answers

type NetworkAnswers = BTreeMap<String, String>;

struct PartitionedAnswers {
    target: NetworkAnswers,
    remaining: NetworkAnswers,
}

// Network configuration

struct NetworkConfig {
    address: &'static str,
    prefix_length: u8,
    dhcp: bool,
    nat: bool,
    host_adapter: bool,
}

impl NetworkConfig {
    fn netmask(&self) -> Ipv4Addr {
        u32::MAX
            .checked_shl(32 - u32::from(self.prefix_length))
            .unwrap_or(0)
            .into()
    }
}

// Test network

struct TestNetwork {
    name: String,
    prefix: String,
    temp_dir: tempfile::TempDir,
}

impl TestNetwork {
    fn allocate() -> Result<Self> {
        let answers = read_answers()?;

        // vmnet1 and vmnet8 are VMware Fusion's default host-only and NAT networks
        let number = (2..=u8::MAX)
            .filter(|number| *number != 8)
            .find(|number| !answers.contains_key(&format!("VNET_{number}_HOSTONLY_SUBNET")))
            .context("could not find an unused VMware network")?;

        Ok(Self {
            name: format!("vmnet{number}"),
            prefix: format!("VNET_{number}_"),
            temp_dir: tempfile::tempdir()?,
        })
    }

    fn config(&self, config: &NetworkConfig) -> Value {
        json!({
            &self.name: {
                "subnet": {
                    "address": config.address,
                    "prefixLength": config.prefix_length,
                },
                "dhcp": { "enable": config.dhcp },
                "nat": { "enable": config.nat },
                "hostAdapter": { "enable": config.host_adapter },
            }
        })
    }

    fn apply(&self, config: &Value) -> Result<()> {
        let path = self.temp_dir.path().join("networks.json");
        fs::write(&path, serde_json::to_vec(config)?)?;

        duct::cmd!(&CONFIG.cli, "network", "apply", &path)
            .env("NIX_VMWARE_FUSION_NETWORK_STATE", self.state_path())
            .run()
            .with_context(|| format!("could not apply configuration for {}", self.name))?;

        Ok(())
    }

    // Inspection

    fn answers(&self) -> Result<PartitionedAnswers> {
        let (target, remaining): (NetworkAnswers, NetworkAnswers) = read_answers()?
            .into_iter()
            .partition(|(key, _)| key.starts_with(&self.prefix));

        let target = target
            .into_iter()
            .filter_map(|(key, value)| {
                key.strip_prefix(&self.prefix)
                    .map(|key| (key.to_owned(), value))
            })
            .collect();

        Ok(PartitionedAnswers { target, remaining })
    }

    fn read_state(&self) -> Result<Value> {
        Ok(serde_json::from_slice(&fs::read(self.state_path())?)?)
    }

    fn exists(&self) -> Result<bool> {
        Ok(self.answers()?.target.contains_key("HOSTONLY_SUBNET"))
    }

    // Paths

    fn state_path(&self) -> PathBuf {
        self.temp_dir.path().join("state.json")
    }

    fn vmnet_path(&self) -> PathBuf {
        Path::new(PREFERENCES_PATH).join(&self.name)
    }

    // Cleanup

    fn remove(&self) -> Result<()> {
        if !self.exists()? {
            return Ok(());
        }

        run_vmnet_cli("--stop")?;
        let remove = run_vmnet_cfgcli(&["deletevnet", &self.name])
            .and_then(|()| run_vmnet_cli("--configure"));
        let restart = run_vmnet_cli("--start");

        remove?;
        restart
    }
}

impl Drop for TestNetwork {
    fn drop(&mut self) {
        let _ = self.remove();
    }
}

// Lifecycle

#[test]
fn cli_manages_system_network_lifecycle() -> Result<()> {
    let network = TestNetwork::allocate()?;

    // Add a network
    let initial = NetworkConfig {
        address: "192.0.2.0",
        prefix_length: 24,
        dhcp: true,
        nat: false,
        host_adapter: true,
    };

    network.apply(&network.config(&initial))?;

    let initial_answers = network.answers()?;

    assert_network_answers(&initial_answers.target, &initial);
    assert_eq!(
        network.read_state()?,
        json!({ "formatVersion": 1, "networks": { &network.name: {} } })
    );

    // Update every supported network setting
    let updated = NetworkConfig {
        address: "198.51.100.0",
        prefix_length: 25,
        dhcp: false,
        nat: true,
        host_adapter: false,
    };

    network.apply(&network.config(&updated))?;

    let answers = network.answers()?.target;
    assert_network_answers(&answers, &updated);

    // Reapply the same configuration without changing either file
    let networking_file_before = fs::read(NETWORKING_PATH)?;
    let state_file_before = fs::read(network.state_path())?;

    network.apply(&network.config(&updated))?;

    assert_eq!(fs::read(NETWORKING_PATH)?, networking_file_before);
    assert_eq!(fs::read(network.state_path())?, state_file_before);

    // Remove the managed network without changing unrelated answers
    network.apply(&json!({}))?;

    assert!(!network.exists()?);
    assert!(!network.vmnet_path().exists());
    assert_eq!(
        network.read_state()?,
        json!({ "formatVersion": 1, "networks": {} })
    );
    assert_eq!(network.answers()?.remaining, initial_answers.remaining);

    Ok(())
}

// Checks

fn assert_network_answers(answers: &NetworkAnswers, expected: &NetworkConfig) {
    let netmask = expected.netmask().to_string();

    assert_eq!(
        answers.get("HOSTONLY_SUBNET").map(String::as_str),
        Some(expected.address)
    );
    assert_eq!(
        answers.get("HOSTONLY_NETMASK").map(String::as_str),
        Some(netmask.as_str())
    );
    assert_eq!(
        answers.get("DHCP").map(String::as_str),
        Some(yes_no(expected.dhcp))
    );

    let nat = answers.get("NAT").map(String::as_str);
    if expected.nat {
        assert_eq!(nat, Some("yes"));
    } else {
        assert!(matches!(nat, None | Some("no")));
    }

    assert_eq!(
        answers.get("VIRTUAL_ADAPTER").map(String::as_str),
        Some(yes_no(expected.host_adapter))
    );
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

// VMware networking

fn read_answers() -> Result<NetworkAnswers> {
    let contents = fs::read_to_string(NETWORKING_PATH)
        .context("could not read VMware Fusion networking file")?;

    Ok(contents
        .lines()
        .filter_map(|line| line.strip_prefix("answer "))
        .filter_map(|line| line.split_once(' '))
        // VMware Fusion regenerates DHCP config hashes when applying networking
        .filter(|(key, _)| !key.ends_with("_DHCP_CFG_HASH"))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect())
}

fn run_vmnet_cli(argument: &str) -> Result<()> {
    duct::cmd!(&CONFIG.vmnet_cli, argument)
        .run()
        .with_context(|| format!("could not run vmnet-cli {argument}"))?;

    Ok(())
}

fn run_vmnet_cfgcli(arguments: &[&str]) -> Result<()> {
    let status = duct::cmd(&CONFIG.vmnet_cfgcli, arguments)
        .unchecked()
        .run()
        .with_context(|| format!("could not run vmnet-cfgcli {}", arguments[0]))?
        .status
        .code();

    ensure!(
        status == Some(1),
        "expected vmnet-cfgcli to report success with status 1, got {status:?}"
    );

    Ok(())
}
