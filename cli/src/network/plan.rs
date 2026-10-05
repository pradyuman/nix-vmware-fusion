use anyhow::Result;

use super::schema::{Network, Networks, VmnetName};
use super::state::{NetworkMetadata, State};
use super::{Action, NetworkAnswers, Plan, Snapshot, yes_no};

pub(super) fn plan(configured: &Networks, snapshot: Snapshot) -> Result<Plan> {
    let Snapshot { state, answers } = snapshot;

    let remove_actions = state
        .networks
        .keys()
        .filter(|name| !configured.contains_key(*name))
        .filter(|name| network_exists(name, &answers))
        .cloned()
        .map(Action::RemoveNetwork)
        .collect::<Vec<_>>();

    let network_actions = configured
        .iter()
        .map(|(name, network)| plan_network(name, network, &answers))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    Ok(Plan {
        state: State {
            networks: configured
                .keys()
                .cloned()
                .map(|name| (name, NetworkMetadata::default()))
                .collect(),
        },
        actions: remove_actions.into_iter().chain(network_actions).collect(),
    })
}

fn plan_network(
    name: &VmnetName,
    network: &Network,
    answers: &NetworkAnswers,
) -> Result<Vec<Action>> {
    let prefix = format!("VNET_{}_", name.number());
    let subnet_mask = network.subnet.netmask()?;

    let exists = network_exists(name, answers);
    let add_action = (!exists).then(|| Action::AddNetwork(name.clone()));

    let set_actions = [
        (
            format!("{prefix}HOSTONLY_SUBNET"),
            network.subnet.address.to_string(),
            Action::SetSubnetAddress(name.clone(), network.subnet.address),
        ),
        (
            format!("{prefix}HOSTONLY_NETMASK"),
            subnet_mask.to_string(),
            Action::SetSubnetMask(name.clone(), subnet_mask),
        ),
        (
            format!("{prefix}DHCP"),
            yes_no(network.dhcp.enable).to_owned(),
            Action::SetDhcp(name.clone(), network.dhcp.enable),
        ),
        (
            format!("{prefix}NAT"),
            yes_no(network.nat.enable).to_owned(),
            Action::SetNat(name.clone(), network.nat.enable),
        ),
        (
            format!("{prefix}VIRTUAL_ADAPTER"),
            yes_no(network.host_adapter.enable).to_owned(),
            Action::SetHostAdapter(name.clone(), network.host_adapter.enable),
        ),
    ]
    .into_iter()
    .filter_map(|(key, expected, action)| {
        let observed = answers.get(&key).map(String::as_str).unwrap_or("no");
        (!exists || observed != expected).then_some(action)
    });

    Ok(add_action.into_iter().chain(set_actions).collect())
}

fn network_exists(name: &VmnetName, answers: &NetworkAnswers) -> bool {
    answers.contains_key(&format!("VNET_{}_HOSTONLY_SUBNET", name.number()))
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;
    use crate::network::schema::{Ipv4PrefixLength, Subnet, Toggle, VmnetName};
    use crate::network::state::ManagedNetworks;

    const NETWORK_ADDRESS: Ipv4Addr = Ipv4Addr::new(192, 0, 2, 0);
    const HOST_ADDRESS: Ipv4Addr = Ipv4Addr::new(192, 0, 2, 1);
    const SUBNET_MASK: Ipv4Addr = Ipv4Addr::new(255, 255, 255, 0);

    fn vmnet_name(name: &str) -> VmnetName {
        VmnetName::try_new(name.to_owned()).unwrap()
    }

    fn network() -> Network {
        Network {
            subnet: Subnet {
                address: NETWORK_ADDRESS,
                prefix_length: Ipv4PrefixLength::try_new(24).unwrap(),
            },
            dhcp: Toggle { enable: true },
            nat: Toggle { enable: false },
            host_adapter: Toggle { enable: true },
        }
    }

    fn snapshot(answers: NetworkAnswers) -> Snapshot {
        Snapshot {
            state: State::default(),
            answers,
        }
    }

    #[test]
    fn plans_missing_network() -> Result<()> {
        let configured = Networks::from([(vmnet_name("vmnet2"), network())]);

        assert_eq!(
            plan(&configured, snapshot(NetworkAnswers::new()))?.actions,
            [
                Action::AddNetwork(vmnet_name("vmnet2")),
                Action::SetSubnetAddress(vmnet_name("vmnet2"), NETWORK_ADDRESS),
                Action::SetSubnetMask(vmnet_name("vmnet2"), SUBNET_MASK),
                Action::SetDhcp(vmnet_name("vmnet2"), true),
                Action::SetNat(vmnet_name("vmnet2"), false),
                Action::SetHostAdapter(vmnet_name("vmnet2"), true),
            ]
        );

        Ok(())
    }

    #[test]
    fn plans_deleted_network_as_missing() -> Result<()> {
        let configured = Networks::from([(vmnet_name("vmnet2"), network())]);
        let snapshot = snapshot(NetworkAnswers::from([
            ("VNET_2_DHCP".to_owned(), "no".to_owned()),
            ("VNET_2_HOSTONLY_UUID".to_owned(), "generated".to_owned()),
            ("VNET_2_NAT".to_owned(), "no".to_owned()),
            ("VNET_2_VIRTUAL_ADAPTER".to_owned(), "no".to_owned()),
        ]));

        assert!(matches!(
            plan(&configured, snapshot)?.actions.first(),
            Some(Action::AddNetwork(name)) if name == &vmnet_name("vmnet2")
        ));

        Ok(())
    }

    #[test]
    fn plans_only_changed_network_settings() -> Result<()> {
        let configured = Networks::from([(vmnet_name("vmnet2"), network())]);
        let snapshot = snapshot(NetworkAnswers::from([
            (
                "VNET_2_HOSTONLY_SUBNET".to_owned(),
                NETWORK_ADDRESS.to_string(),
            ),
            (
                "VNET_2_HOSTONLY_NETMASK".to_owned(),
                SUBNET_MASK.to_string(),
            ),
            ("VNET_2_DHCP".to_owned(), "yes".to_owned()),
            ("VNET_2_NAT".to_owned(), "yes".to_owned()),
            ("VNET_2_VIRTUAL_ADAPTER".to_owned(), "yes".to_owned()),
            ("VNET_2_HOSTONLY_UUID".to_owned(), "generated".to_owned()),
        ]));

        assert_eq!(
            plan(&configured, snapshot)?.actions,
            [Action::SetNat(vmnet_name("vmnet2"), false)]
        );

        Ok(())
    }

    #[test]
    fn omitted_disabled_setting_is_unchanged() -> Result<()> {
        let configured = Networks::from([(vmnet_name("vmnet2"), network())]);
        let snapshot = snapshot(NetworkAnswers::from([
            (
                "VNET_2_HOSTONLY_SUBNET".to_owned(),
                NETWORK_ADDRESS.to_string(),
            ),
            (
                "VNET_2_HOSTONLY_NETMASK".to_owned(),
                SUBNET_MASK.to_string(),
            ),
            ("VNET_2_DHCP".to_owned(), "yes".to_owned()),
            ("VNET_2_VIRTUAL_ADAPTER".to_owned(), "yes".to_owned()),
        ]));

        assert!(plan(&configured, snapshot)?.actions.is_empty());

        Ok(())
    }

    #[test]
    fn rejects_non_network_address() {
        let mut network = network();
        network.subnet.address = HOST_ADDRESS;
        let configured = Networks::from([(vmnet_name("vmnet2"), network)]);

        let error = plan(&configured, snapshot(NetworkAnswers::new()))
            .expect_err("host address should not be accepted as a subnet");

        assert!(error.to_string().contains("is not a /24 network address"));
    }

    #[test]
    fn records_declared_network_as_managed() -> Result<()> {
        let name = vmnet_name("vmnet2");
        let configured = Networks::from([(name.clone(), network())]);

        let plan = plan(&configured, snapshot(NetworkAnswers::new()))?;

        assert_eq!(
            plan.state.networks,
            ManagedNetworks::from([(name, NetworkMetadata::default())])
        );

        Ok(())
    }

    #[test]
    fn removes_undeclared_managed_network() -> Result<()> {
        let name = vmnet_name("vmnet2");
        let mut snapshot = snapshot(NetworkAnswers::from([(
            "VNET_2_HOSTONLY_SUBNET".to_owned(),
            NETWORK_ADDRESS.to_string(),
        )]));
        snapshot
            .state
            .networks
            .insert(name.clone(), NetworkMetadata::default());

        let plan = plan(&Networks::new(), snapshot)?;

        assert_eq!(plan.actions, [Action::RemoveNetwork(name)]);
        assert!(plan.state.networks.is_empty());

        Ok(())
    }

    #[test]
    fn ignores_undeclared_unmanaged_network() -> Result<()> {
        let snapshot = snapshot(NetworkAnswers::from([(
            "VNET_2_HOSTONLY_SUBNET".to_owned(),
            NETWORK_ADDRESS.to_string(),
        )]));

        let plan = plan(&Networks::new(), snapshot)?;

        assert!(plan.actions.is_empty());

        Ok(())
    }
}
