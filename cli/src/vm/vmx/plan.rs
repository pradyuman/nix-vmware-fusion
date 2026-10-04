use anyhow::{Result, ensure};

use crate::vm::schema::VirtualMachine;

use super::{Action, Plan, Snapshot};

pub(crate) fn plan(schema: &VirtualMachine, snapshot: Snapshot) -> Result<Plan> {
    let mut actions = vec![
        Action::Set("displayName", schema.display_name.clone()),
        Action::Set("guestOS", schema.guest_os.clone()),
        Action::Set("numvcpus", schema.vcpus.to_string()),
        plan_cpu_topology(schema)?,
        Action::Set("memsize", schema.memory.to_string()),
        Action::Set(
            "uefi.secureBoot.enabled",
            if schema.secure_boot { "TRUE" } else { "FALSE" }.to_owned(),
        ),
    ];

    if snapshot.raw_contents.is_none() {
        // Fusion on Apple silicon requires UEFI; BIOS is unsupported
        // https://knowledge.broadcom.com/external/article/315602
        actions.push(Action::Set("firmware", "efi".to_owned()));

        // Enable keyboard and mouse input through the virtual XHCI controller
        actions.push(Action::Set("usb_xhci.present", "TRUE".to_owned()));
    }

    Ok(Plan {
        snapshot,
        guest_os: schema.guest_os.clone(),
        actions,
    })
}

fn plan_cpu_topology(schema: &VirtualMachine) -> Result<Action> {
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
