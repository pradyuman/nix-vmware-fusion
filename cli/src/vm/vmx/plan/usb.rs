use crate::vm::{
    schema::{Usb, UsbNewDeviceAction},
    vmx::Action,
};

const MANAGED_KEYS: [&str; 4] = [
    "usb.present",
    "ehci.present",
    "usb_xhci.present",
    "usb.generic.pluginAction",
];

impl UsbNewDeviceAction {
    fn vmx_value(self) -> Option<&'static str> {
        match self {
            Self::Ask => None,
            Self::ConnectToVm => Some("guest"),
            Self::ConnectToHost => Some("host"),
        }
    }
}

pub(super) fn plan(configured: Option<&Usb>) -> [Action; 4] {
    configured.map_or_else(
        || MANAGED_KEYS.map(Action::Remove),
        |configured| {
            [
                Action::Set("usb.present", "TRUE".to_owned()),
                Action::Set("ehci.present", "TRUE".to_owned()),
                Action::Set("usb_xhci.present", "TRUE".to_owned()),
                configured.new_device_action.vmx_value().map_or_else(
                    || Action::Remove("usb.generic.pluginAction"),
                    |value| Action::Set("usb.generic.pluginAction", value.to_owned()),
                ),
            ]
        },
    )
}
