use std::collections::BTreeMap;
use std::net::Ipv4Addr;

use nutype::nutype;
use serde::Deserialize;

pub(crate) type Networks = BTreeMap<VmnetName, Network>;

#[nutype(
    validate(predicate = |name| parse_vmnet_number(name).is_some()),
    derive(Clone, Debug, Deserialize, Serialize, AsRef, Display, Eq, Ord, PartialEq, PartialOrd)
)]
pub(crate) struct VmnetName(String);

impl VmnetName {
    pub(crate) fn number(&self) -> u8 {
        parse_vmnet_number(self.as_ref()).expect("validated VMware network name")
    }
}

fn parse_vmnet_number(name: &str) -> Option<u8> {
    let number = name.strip_prefix("vmnet")?.parse().ok()?;

    // vmnet0 is reserved for bridged networking
    (number != 0 && name == format!("vmnet{number}")).then_some(number)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Network {
    pub subnet: Subnet,
    pub dhcp: Toggle,
    pub nat: Toggle,
    pub host_adapter: Toggle,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Subnet {
    pub address: Ipv4Addr,
    pub prefix_length: Ipv4PrefixLength,
}

#[nutype(
    validate(less_or_equal = 32),
    derive(Clone, Copy, Debug, Deserialize, Into)
)]
pub(crate) struct Ipv4PrefixLength(u8);

#[derive(Debug, Deserialize)]
pub(crate) struct Toggle {
    pub enable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_private_network_names() {
        assert!(VmnetName::try_new("vmnet0".to_owned()).is_err());
        assert!(VmnetName::try_new("vmnet01".to_owned()).is_err());
    }
}
