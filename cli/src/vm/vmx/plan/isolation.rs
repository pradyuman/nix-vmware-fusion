use crate::vm::{schema::Isolation, vmx::Action};

use super::boolean;

pub(super) fn plan(configured: &Isolation) -> [Action; 3] {
    [
        boolean("isolation.tools.copy.disable", !configured.clipboard, false),
        boolean(
            "isolation.tools.paste.disable",
            !configured.clipboard,
            false,
        ),
        boolean(
            "isolation.tools.dnd.disable",
            !configured.drag_and_drop,
            false,
        ),
    ]
}
