use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

use crate::config::CONFIG;

use super::{NetworkAdapterLabel, NetworkAttachment, Snapshot};

#[derive(Deserialize)]
struct VmcliNetworkQuery {
    devices: Vec<VmcliNetworkAdapter>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VmcliNetworkAdapter {
    label: NetworkAdapterLabel,
    external_id: String,
    connection_type: String,
    vnet: String,
    virtual_dev: String,
    start_connected: bool,
}

pub(crate) fn inspect(vmx_path: &Path) -> Result<Snapshot> {
    Ok(Snapshot {
        network_attachments: query_network_attachments(vmx_path)?,
    })
}

fn query_network_attachments(vmx_path: &Path) -> Result<Vec<NetworkAttachment>> {
    if !vmx_path.try_exists()? {
        return Ok(Vec::new());
    }

    let json = duct::cmd!(&CONFIG.vmcli, vmx_path, "ethernet", "query", "-f", "json")
        .read()
        .context("could not query network adapters")?;

    // vmcli prefixes an empty device response with a human-readable message
    let json_start = json
        .find('{')
        .context("vmcli did not report network adapter JSON")?;
    let query = serde_json::from_str::<VmcliNetworkQuery>(&json[json_start..])
        .context("could not parse vmcli network adapter JSON")?;

    Ok(query
        .devices
        .into_iter()
        .map(|adapter| NetworkAttachment {
            label: adapter.label,
            external_id: adapter.external_id,
            mode: adapter.connection_type,
            vmnet: adapter.vnet,
            model: adapter.virtual_dev,
            start_connected: adapter.start_connected,
        })
        .collect())
}

#[cfg(all(test, feature = "vmware-tests"))]
mod tests {
    use crate::vm::test_support::create_vmx;

    use super::*;

    fn set_adapter_property(path: &Path, command: &str, value: &str) -> Result<()> {
        duct::cmd!(&CONFIG.vmcli, path, "ethernet", command, "ethernet0", value).run()?;

        Ok(())
    }

    #[test]
    fn vmcli_reports_no_network_attachments() -> Result<()> {
        let (_temp_dir, vmx_path) = create_vmx()?;

        assert!(query_network_attachments(&vmx_path)?.is_empty());

        Ok(())
    }

    #[test]
    fn vmcli_reports_network_attachment() -> Result<()> {
        let (_temp_dir, vmx_path) = create_vmx()?;

        set_adapter_property(&vmx_path, "setpresent", "true")?;
        set_adapter_property(&vmx_path, "setconnectiontype", "custom")?;
        duct::cmd!(
            &CONFIG.vmcli,
            &vmx_path,
            "ethernet",
            "setcustomtypebacking",
            "ethernet0",
            "vmnet2",
            "",
            ""
        )
        .run()?;
        set_adapter_property(&vmx_path, "setvirtualdevice", "e1000e")?;
        set_adapter_property(&vmx_path, "setstartconnected", "false")?;
        set_adapter_property(&vmx_path, "setexternalid", "nix-vmware-fusion:primary")?;

        let attachments = query_network_attachments(&vmx_path)?;

        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].label, "ethernet0");
        assert_eq!(attachments[0].external_id, "nix-vmware-fusion:primary");
        assert_eq!(attachments[0].mode, "custom");
        assert_eq!(attachments[0].vmnet, "vmnet2");
        assert_eq!(attachments[0].model, "e1000e");
        assert!(!attachments[0].start_connected);

        Ok(())
    }
}
