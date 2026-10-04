use std::collections::BTreeMap;
use std::net::Ipv4Addr;

use anyhow::{Result, ensure};
use nutype::nutype;
use serde::Deserialize;

pub(super) type Networks = BTreeMap<VmnetName, Network>;

#[nutype(
    validate(predicate = |name| parse_vmnet_number(name).is_some()),
    derive(Clone, Debug, Deserialize, Serialize, AsRef, Display, Eq, Ord, PartialEq, PartialOrd)
)]
pub(super) struct VmnetName(String);

impl VmnetName {
    pub(super) fn number(&self) -> u8 {
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
pub(super) struct Network {
    pub subnet: Subnet,
    pub dhcp: Toggle,
    pub nat: Toggle,
    pub host_adapter: Toggle,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Subnet {
    pub address: Ipv4Addr,
    pub prefix_length: Ipv4PrefixLength,
}

impl Subnet {
    pub(super) fn netmask(&self) -> Result<Ipv4Addr> {
        let netmask = self.prefix_length.netmask();
        let address = u32::from(self.address);

        // A network address cannot have host bits set outside its prefix
        ensure!(
            address & u32::from(netmask) == address,
            "{} is not a /{} network address",
            self.address,
            u8::from(self.prefix_length)
        );

        Ok(netmask)
    }
}

#[nutype(
    validate(less_or_equal = 32),
    derive(Clone, Copy, Debug, Deserialize, Into)
)]
pub(super) struct Ipv4PrefixLength(u8);

impl Ipv4PrefixLength {
    fn netmask(self) -> Ipv4Addr {
        let prefix_length = u8::from(self);

        // A /0 prefix's shift by 32 returns None, so it unwraps to a zero mask
        u32::MAX
            .checked_shl(32 - u32::from(prefix_length))
            .unwrap_or(0)
            .into()
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct Toggle {
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
