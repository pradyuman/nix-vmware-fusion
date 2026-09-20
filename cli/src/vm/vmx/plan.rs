use crate::vm::schema::VirtualMachine;

use super::{Plan, Snapshot};

pub(crate) fn plan(schema: &VirtualMachine, snapshot: Snapshot) -> Plan {
    let mut entries = vec![
        ("displayName", schema.display_name.clone()),
        ("guestOS", schema.guest_os.clone()),
        ("numvcpus", schema.vcpus.to_string()),
        ("memsize", schema.memory.to_string()),
        (
            "uefi.secureBoot.enabled",
            if schema.secure_boot { "TRUE" } else { "FALSE" }.to_owned(),
        ),
    ];

    if snapshot.raw_contents.is_none() {
        // Fusion on Apple silicon requires UEFI; BIOS is unsupported
        // https://knowledge.broadcom.com/external/article/315602
        entries.push(("firmware", "efi".to_owned()));
    }

    Plan {
        snapshot,
        guest_os: schema.guest_os.clone(),
        entries,
    }
}
