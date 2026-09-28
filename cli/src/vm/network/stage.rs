use anyhow::{Context, Result};
use std::ffi::OsStr;
use std::path::Path;

use crate::config::CONFIG;
use crate::vm::schema::{NetworkAdapter, NetworkMode};

use super::{Action, Plan, external_id};

pub(crate) fn stage(draft_path: &Path, plan: Plan) -> Result<()> {
    plan.actions
        .into_iter()
        .try_for_each(|action| match action {
            Action::Configure {
                label,
                name,
                adapter,
            } => configure(draft_path, &label, &name, &adapter),
            Action::Remove { label } => {
                duct::cmd!(&CONFIG.vmcli, draft_path, "ethernet", "purge", &label)
                    .run()
                    .with_context(|| format!("could not remove network adapter {label}"))
                    .map(|_| ())
            }
        })
}

fn configure(path: &Path, label: &str, name: &str, adapter: &NetworkAdapter) -> Result<()> {
    set(path, label, "setpresent", "true")?;
    set(path, label, "setconnectiontype", adapter.mode.vmcli_value())?;

    if adapter.mode == NetworkMode::Custom {
        duct::cmd!(
            &CONFIG.vmcli,
            path,
            "ethernet",
            "setcustomtypebacking",
            label,
            adapter.vmnet.as_deref().expect("validated custom vmnet"),
            "",
            ""
        )
        .run()
        .with_context(|| format!("could not set custom vmnet for {label}"))?;
    }

    set(path, label, "setvirtualdevice", adapter.model.vmcli_value())?;
    set(
        path,
        label,
        "setstartconnected",
        adapter.start_connected.to_string(),
    )?;
    set(path, label, "setexternalid", external_id(name))?;

    Ok(())
}

fn set(path: &Path, label: &str, command: &str, value: impl AsRef<OsStr>) -> Result<()> {
    duct::cmd!(
        &CONFIG.vmcli,
        path,
        "ethernet",
        command,
        label,
        value.as_ref()
    )
    .run()
    .with_context(|| format!("could not run {command} for network adapter {label}"))?;

    Ok(())
}

#[cfg(all(test, feature = "vmware-tests"))]
mod tests {
    use crate::vm::network::inspect;
    use crate::vm::schema::NetworkAdapterModel;
    use crate::vm::test_support::create_vmx;

    use super::*;

    #[test]
    fn vmcli_configures_and_removes_network_adapter() -> Result<()> {
        let (_temp_dir, vmx_path) = create_vmx()?;
        let label = "ethernet0";
        let name = "primary";

        stage(
            &vmx_path,
            Plan {
                actions: vec![Action::Configure {
                    label: label.to_owned(),
                    name: name.to_owned(),
                    adapter: NetworkAdapter {
                        mode: NetworkMode::Custom,
                        vmnet: Some("vmnet2".to_owned()),
                        model: NetworkAdapterModel::E1000e,
                        start_connected: false,
                    },
                }],
            },
        )?;

        let snapshot = inspect(&vmx_path)?;
        assert_eq!(snapshot.network_attachments.len(), 1);

        let attachment = &snapshot.network_attachments[0];
        assert_eq!(attachment.label, label);
        assert_eq!(attachment.external_id, external_id(name));
        assert_eq!(attachment.mode, "custom");
        assert_eq!(attachment.vmnet, "vmnet2");
        assert_eq!(attachment.model, "e1000e");
        assert!(!attachment.start_connected);

        stage(
            &vmx_path,
            Plan {
                actions: vec![Action::Remove {
                    label: label.to_owned(),
                }],
            },
        )?;

        assert!(inspect(&vmx_path)?.network_attachments.is_empty());

        Ok(())
    }
}
