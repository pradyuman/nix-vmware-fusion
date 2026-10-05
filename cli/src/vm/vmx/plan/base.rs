use crate::vm::{
    schema::VirtualMachine,
    vmx::{Action, Snapshot},
};

pub(super) fn plan(configured: &VirtualMachine, snapshot: &Snapshot) -> Vec<Action> {
    let mut actions = vec![
        Action::Set("displayName", configured.display_name.clone()),
        Action::Set("guestOS", configured.guest_os.clone()),
        Action::Set("memsize", configured.memory.to_string()),
        Action::Set(
            "uefi.secureBoot.enabled",
            if configured.secure_boot {
                "TRUE"
            } else {
                "FALSE"
            }
            .to_owned(),
        ),
    ];

    if snapshot.raw_contents.is_none() {
        // Fusion on Apple silicon requires UEFI; BIOS is unsupported
        // https://knowledge.broadcom.com/external/article/315602
        actions.push(Action::Set("firmware", "efi".to_owned()));
    }

    actions
}
