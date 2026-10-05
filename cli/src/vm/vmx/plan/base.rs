use crate::vm::{
    schema::VirtualMachine,
    vmx::{Action, Snapshot},
};

pub(super) fn plan(schema: &VirtualMachine, snapshot: &Snapshot) -> Vec<Action> {
    let mut actions = vec![
        Action::Set("displayName", schema.display_name.clone()),
        Action::Set("guestOS", schema.guest_os.clone()),
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

    actions
}
