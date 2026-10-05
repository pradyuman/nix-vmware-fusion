use anyhow::{Result, ensure};

use crate::vm::{schema::VirtualMachine, vmx::Action};

pub(super) fn plan(configured: &VirtualMachine) -> Result<[Action; 2]> {
    Ok([
        Action::Set("numvcpus", configured.vcpus.to_string()),
        plan_topology(configured)?,
    ])
}

fn plan_topology(configured: &VirtualMachine) -> Result<Action> {
    configured.cores_per_socket.map_or_else(
        || Ok(Action::Remove("cpuid.coresPerSocket")),
        |cores| {
            ensure!(
                configured.vcpus.get().is_multiple_of(cores.get()),
                "coresPerSocket must evenly divide vcpus"
            );

            Ok(Action::Set("cpuid.coresPerSocket", cores.to_string()))
        },
    )
}
