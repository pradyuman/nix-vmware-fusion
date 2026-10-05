use crate::vm::schema::{Display, ScaledHighResolution};

use super::{Action, Plan, Snapshot};

impl ScaledHighResolution {
    fn plist_value(self) -> Option<u8> {
        match self {
            Self::FullScreen => Some(1),
            Self::SingleWindow => Some(2),
            Self::All => None,
        }
    }
}

pub(crate) fn plan(configured: Option<&Display>, snapshot: Snapshot) -> Plan {
    let value = configured.and_then(|configured| {
        configured
            .native_display_resolution
            .scaled_high_resolution
            .plist_value()
    });
    let action = value.map_or(
        Action::RemoveScaledHighResolution,
        Action::SetScaledHighResolution,
    );

    Plan { snapshot, action }
}
