use std::ffi::OsString;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::config::CONFIG;

#[derive(Deserialize)]
struct FreeLabel<L> {
    #[serde(rename = "FindFirstFree")]
    label: L,
}

// Run a device command that takes `label` and one value, e.g. `ethernet setpresent ethernet0 true`
pub(super) fn run(
    vmx_path: &Path,
    module: &str,
    command: &str,
    label: impl AsRef<str>,
    value: impl Into<OsString>,
) -> Result<()> {
    let label = label.as_ref();

    duct::cmd!(&CONFIG.vmcli, vmx_path, module, command, label, value)
        .run()
        .with_context(|| format!("could not run vmcli {module} {command} for {label}"))?;

    Ok(())
}

// Query every device in `module`, e.g. `disk query`
pub(super) fn query<T: DeserializeOwned>(vmx_path: &Path, module: &str) -> Result<T> {
    let output = duct::cmd!(&CONFIG.vmcli, vmx_path, module, "query", "-f", "json")
        .read()
        .with_context(|| format!("could not run vmcli {module} query"))?;

    // vmcli prefixes empty ethernet and HGFS responses with a human-readable message
    let json_start = output
        .find('{')
        .with_context(|| format!("vmcli did not report {module} JSON"))?;

    serde_json::from_str(&output[json_start..])
        .with_context(|| format!("could not parse vmcli {module} JSON"))
}

// Remove every VMX entry belonging to the device identified by `label`
pub(super) fn purge(vmx_path: &Path, module: &str, label: impl AsRef<str>) -> Result<()> {
    let label = label.as_ref();

    duct::cmd!(&CONFIG.vmcli, vmx_path, module, "purge", label)
        .run()
        .with_context(|| format!("could not run vmcli {module} purge for {label}"))?;

    Ok(())
}

pub(super) fn set_backing_info(
    vmx_path: &Path,
    label: impl AsRef<str>,
    backing_type: &str,
    backing_path: impl Into<OsString>,
    client_device: bool,
) -> Result<()> {
    let label = label.as_ref();

    duct::cmd!(
        &CONFIG.vmcli,
        vmx_path,
        "disk",
        "setbackinginfo",
        label,
        backing_type,
        backing_path,
        client_device.to_string()
    )
    .run()
    .with_context(|| format!("could not set {backing_type} backing for {label}"))?;

    Ok(())
}

pub(super) fn set_custom_vmnet(vmx_path: &Path, label: impl AsRef<str>, vmnet: &str) -> Result<()> {
    let label = label.as_ref();

    duct::cmd!(
        &CONFIG.vmcli,
        vmx_path,
        "ethernet",
        "setcustomtypebacking",
        label,
        vmnet,
        "",
        ""
    )
    .run()
    .with_context(|| format!("could not set custom vmnet for {label}"))?;

    Ok(())
}

pub(super) fn enable_controller(vmx_path: &Path, module: &str, controller: &str) -> Result<()> {
    run(vmx_path, module, "setpresent", controller, "true")
        .with_context(|| format!("could not enable {controller}"))
}

// Find the first unused device label on `controller`, e.g. `sata0:1` on `sata0`
pub(super) fn find_first_free<L: DeserializeOwned>(
    vmx_path: &Path,
    module: &str,
    controller: &str,
) -> Result<L> {
    let json = duct::cmd!(
        &CONFIG.vmcli,
        vmx_path,
        module,
        "findfirstfree",
        controller,
        "-f",
        "json"
    )
    .read()
    .with_context(|| format!("could not find a free label on {controller}"))?;

    Ok(serde_json::from_str::<FreeLabel<L>>(&json)
        .context("could not parse vmcli free label JSON")?
        .label)
}

pub(super) fn create(vmx_path: &Path, guest_os: &str) -> Result<()> {
    let name = vmx_path
        .file_stem()
        .context("missing virtual machine name")?;
    let directory_path = vmx_path.parent().context("missing VMX directory")?;

    duct::cmd!(
        &CONFIG.vmcli,
        "vm",
        "create",
        "-n",
        name,
        "-d",
        directory_path,
        "-c",
        guest_os
    )
    .run()
    .context("could not create virtual machine")?;

    Ok(())
}

pub(super) fn power_state(vmx_path: &Path) -> Result<String> {
    let output = duct::cmd!(&CONFIG.vmcli, vmx_path, "power", "query")
        .read()
        .context("could not query virtual machine power state")?;

    output
        .lines()
        .find_map(|line| line.strip_prefix("PowerState:"))
        .map(|state| state.trim().to_owned())
        .context("vmcli did not report a power state")
}
