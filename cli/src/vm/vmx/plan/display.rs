use crate::vm::{
    schema::{Display, FullScreenFit, SingleWindowFit},
    vmx::Action,
};

use super::{boolean, optional};

const MANAGED_KEYS: [&str; 6] = [
    "mks.enable3d",
    "svga.graphicsMemoryKB",
    "gui.perVMWindowAutofitMode",
    "gui.perVMFullscreenAutofitMode",
    "gui.fullScreenOnAllHostDisplays",
    "gui.fitGuestUsingNativeDisplayResolution",
];

impl SingleWindowFit {
    fn vmx_value(self) -> Option<&'static str> {
        match self {
            Self::Inherit => None,
            Self::Stretch => Some("stretch"),
            Self::Resize => Some("resize"),
        }
    }
}

impl FullScreenFit {
    fn vmx_value(self) -> Option<&'static str> {
        match self {
            Self::Inherit => None,
            Self::Center => Some("center"),
            Self::Stretch => Some("stretch"),
            Self::Resize => Some("resize"),
        }
    }
}

pub(super) fn plan(configured: Option<&Display>) -> [Action; 6] {
    configured.map_or_else(
        || MANAGED_KEYS.map(Action::Remove),
        |configured| {
            [
                boolean("mks.enable3d", configured.graphics.accelerate_3d, false),
                graphics_memory(configured),
                optional(
                    "gui.perVMWindowAutofitMode",
                    configured.single_window_fit.vmx_value(),
                ),
                optional(
                    "gui.perVMFullscreenAutofitMode",
                    configured.full_screen_fit.vmx_value(),
                ),
                boolean(
                    "gui.fullScreenOnAllHostDisplays",
                    configured.use_all_displays_in_full_screen,
                    false,
                ),
                boolean(
                    "gui.fitGuestUsingNativeDisplayResolution",
                    configured.native_display_resolution.enable,
                    false,
                ),
            ]
        },
    )
}

fn graphics_memory(display: &Display) -> Action {
    Action::Set(
        "svga.graphicsMemoryKB",
        (display.graphics.memory.get() * 1024).to_string(),
    )
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use crate::vm::schema::{DisplayGraphics, NativeDisplayResolution, ScaledHighResolution};

    use super::*;

    #[test]
    fn display_settings_are_planned() {
        let display = Display {
            graphics: DisplayGraphics {
                accelerate_3d: false,
                memory: NonZeroU64::new(515).unwrap(),
            },
            native_display_resolution: NativeDisplayResolution {
                enable: true,
                scaled_high_resolution: ScaledHighResolution::All,
            },
            single_window_fit: SingleWindowFit::Stretch,
            full_screen_fit: FullScreenFit::Center,
            use_all_displays_in_full_screen: true,
        };

        assert_eq!(
            plan(Some(&display)),
            [
                Action::Remove("mks.enable3d"),
                Action::Set("svga.graphicsMemoryKB", "527360".to_owned()),
                Action::Set("gui.perVMWindowAutofitMode", "stretch".to_owned()),
                Action::Set("gui.perVMFullscreenAutofitMode", "center".to_owned()),
                Action::Set("gui.fullScreenOnAllHostDisplays", "TRUE".to_owned()),
                Action::Set(
                    "gui.fitGuestUsingNativeDisplayResolution",
                    "TRUE".to_owned(),
                ),
            ]
        );
    }

    #[test]
    fn absent_display_configuration_removes_managed_entries() {
        assert_eq!(plan(None), MANAGED_KEYS.map(Action::Remove));
    }
}
