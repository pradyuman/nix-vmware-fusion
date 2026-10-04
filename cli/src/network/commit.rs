use std::ffi::OsString;

use anyhow::{Context, Result, bail};

use crate::config::CONFIG;

use super::{Action, Plan, yes_no};

pub(super) fn commit(plan: Plan) -> Result<()> {
    let Plan { state, actions } = plan;

    if actions.is_empty() {
        return state.save();
    }

    ensure_stopped()?;

    println!("Updating VMware Fusion's networking configuration...");
    run_vmnet_cli("--stop")?;

    let update_result = actions
        .into_iter()
        .try_for_each(apply_action)
        .and_then(|()| {
            println!("Applying VMware Fusion's networking configuration...");
            run_vmnet_cli("--configure")
        });
    let restart_result = run_vmnet_cli("--start");

    update_result?;
    restart_result?;
    state.save()
}

fn run_vmnet_cli(argument: &str) -> Result<()> {
    duct::cmd!(&CONFIG.vmnet_cli, argument)
        .run()
        .with_context(|| format!("could not run vmnet-cli {argument}"))?;

    Ok(())
}

fn ensure_stopped() -> Result<()> {
    let output = duct::cmd!("/usr/bin/pgrep", "-x", "vmware-vmx")
        .unchecked()
        .run()
        .context("could not check for running VMware Fusion virtual machines")?;

    match output.status.code() {
        Some(0) => bail!(
            "a VMware Fusion virtual machine is running. Shut it down before updating networking."
        ),
        Some(1) => Ok(()),
        Some(code) => bail!("pgrep failed with status {code}"),
        None => bail!("pgrep was terminated by a signal"),
    }
}

fn apply_action(action: Action) -> Result<()> {
    let arguments: Vec<OsString> = match action {
        Action::AddNetwork(name) => vec!["addadapter".into(), name.to_string().into()],
        Action::RemoveNetwork(name) => vec!["deletevnet".into(), name.to_string().into()],
        Action::SetSubnetAddress(name, address) => {
            vec![
                "setsubnetaddr".into(),
                name.to_string().into(),
                address.to_string().into(),
            ]
        }
        Action::SetSubnetMask(name, mask) => {
            vec![
                "setsubnetmask".into(),
                name.to_string().into(),
                mask.to_string().into(),
            ]
        }
        Action::SetDhcp(name, enabled) => {
            vec![
                "setdhcpusage".into(),
                name.to_string().into(),
                yes_no(enabled).into(),
            ]
        }
        Action::SetNat(name, enabled) => {
            vec![
                "setnatusage".into(),
                name.to_string().into(),
                yes_no(enabled).into(),
            ]
        }
        Action::SetHostAdapter(name, enabled) => vec![
            "vnetcfgadd".into(),
            format!("VNET_{}_VIRTUAL_ADAPTER", name.number()).into(),
            yes_no(enabled).into(),
        ],
    };

    let command = arguments[0].to_string_lossy();
    let output = duct::cmd(&CONFIG.vmnet_cfgcli, &arguments)
        .unchecked()
        .run()
        .with_context(|| format!("could not run vmnet-cfgcli {command}"))?;

    match output.status.code() {
        Some(1) => Ok(()),
        Some(code) => bail!("vmnet-cfgcli {command} failed with status {code}"),
        None => bail!("vmnet-cfgcli {command} was terminated by a signal"),
    }
}
