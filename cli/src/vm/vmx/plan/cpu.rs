use anyhow::{Result, ensure};

use crate::vm::{schema::VirtualMachine, vmx::Action};

pub(super) fn plan(schema: &VirtualMachine) -> Result<[Action; 2]> {
    Ok([
        Action::Set("numvcpus", schema.vcpus.to_string()),
        plan_topology(schema)?,
    ])
}

fn plan_topology(schema: &VirtualMachine) -> Result<Action> {
    schema.cores_per_socket.map_or_else(
        || Ok(Action::Remove("cpuid.coresPerSocket")),
        |cores| {
            ensure!(
                schema.vcpus.get().is_multiple_of(cores.get()),
                "coresPerSocket must evenly divide vcpus"
            );

            Ok(Action::Set("cpuid.coresPerSocket", cores.to_string()))
        },
    )
}
