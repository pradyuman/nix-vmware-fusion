use crate::vm::schema::{NetworkAdapter, NetworkAdapterModel, NetworkMode};

mod inspect;
pub(super) use inspect::inspect;

mod plan;
pub(super) use plan::plan;

mod stage;
pub(super) use stage::stage;

type NetworkAdapterLabel = String;

// Inspect

#[derive(Debug)]
pub(super) struct Snapshot {
    pub network_attachments: Vec<NetworkAttachment>,
}

#[derive(Debug)]
pub(super) struct NetworkAttachment {
    pub label: NetworkAdapterLabel,
    pub external_id: String,
    pub mode: String,
    vmnet: String,
    model: String,
    start_connected: bool,
}

// Plan

#[derive(Debug)]
pub(super) struct Plan {
    actions: Vec<Action>,
}

#[derive(Debug)]
enum Action {
    Configure {
        label: NetworkAdapterLabel,
        name: String,
        adapter: NetworkAdapter,
    },
    Remove {
        label: NetworkAdapterLabel,
    },
}

impl NetworkMode {
    fn vmcli_value(self) -> &'static str {
        match self {
            Self::Nat => "nat",
            Self::Bridged => "bridged",
            Self::HostOnly => "hostonly",
            Self::Custom => "custom",
        }
    }
}

impl NetworkAdapterModel {
    fn vmcli_value(self) -> &'static str {
        match self {
            Self::Vmxnet3 => "vmxnet3",
            Self::E1000e => "e1000e",
            Self::E1000 => "e1000",
        }
    }
}

fn external_id(name: &str) -> String {
    format!("nix-vmware-fusion:{name}")
}
