use super::boolean;
use crate::vm::schema::Isolation;
use crate::vm::vmx::Action;

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
