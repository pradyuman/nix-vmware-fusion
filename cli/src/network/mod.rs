use std::collections::BTreeMap;
use std::fs;
use std::net::Ipv4Addr;
use std::path::Path;

use anyhow::Result;

use self::schema::VmnetName;

mod commit;
mod inspect;
mod plan;
mod schema;
mod state;

// Inspect

type NetworkAnswers = BTreeMap<String, String>;

#[derive(Debug)]
struct Snapshot {
    state: state::State,
    answers: NetworkAnswers,
}

// Plan

#[derive(Debug)]
struct Plan {
    state: state::State,
    actions: Vec<Action>,
}

#[derive(Debug, PartialEq)]
enum Action {
    AddNetwork(VmnetName),
    RemoveNetwork(VmnetName),
    SetSubnetAddress(VmnetName, Ipv4Addr),
    SetSubnetMask(VmnetName, Ipv4Addr),
    SetDhcp(VmnetName, bool),
    SetNat(VmnetName, bool),
    SetHostAdapter(VmnetName, bool),
}

// Apply

pub(crate) fn apply(path: &Path) -> Result<()> {
    let json = fs::read_to_string(path)?;
    let configured = serde_json::from_str::<schema::Networks>(&json)?;

    let snapshot = inspect::inspect()?;
    let plan = plan::plan(&configured, snapshot)?;

    commit::commit(plan)
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
