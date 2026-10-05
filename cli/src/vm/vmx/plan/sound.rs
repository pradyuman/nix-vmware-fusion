use crate::vm::{schema::SoundCard, vmx::Action};

use super::boolean;

const MANAGED_KEYS: [&str; 7] = [
    "sound.present",
    "sound.virtualDev",
    "sound.autoDetect",
    "sound.fileName",
    "sound.startConnected",
    "sound.enableAEC",
    "sound.pciSlotNumber",
];

pub(super) fn plan(configured: Option<&SoundCard>) -> Vec<Action> {
    configured.map_or_else(
        || MANAGED_KEYS.into_iter().map(Action::Remove).collect(),
        |configured| {
            vec![
                Action::Set("sound.present", "TRUE".to_owned()),
                Action::Set("sound.virtualDev", "hdaudio".to_owned()),
                Action::Set("sound.autoDetect", "TRUE".to_owned()),
                Action::Set("sound.fileName", "-1".to_owned()),
                boolean("sound.startConnected", configured.start_connected, true),
                boolean("sound.enableAEC", configured.echo_cancellation, false),
            ]
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sound_card_settings_are_planned() {
        let sound = SoundCard {
            start_connected: false,
            echo_cancellation: true,
        };

        assert_eq!(
            plan(Some(&sound)),
            [
                Action::Set("sound.present", "TRUE".to_owned()),
                Action::Set("sound.virtualDev", "hdaudio".to_owned()),
                Action::Set("sound.autoDetect", "TRUE".to_owned()),
                Action::Set("sound.fileName", "-1".to_owned()),
                Action::Set("sound.startConnected", "FALSE".to_owned()),
                Action::Set("sound.enableAEC", "TRUE".to_owned()),
            ]
        );
    }

    #[test]
    fn absent_sound_card_removes_managed_entries() {
        assert_eq!(plan(None), MANAGED_KEYS.map(Action::Remove));
    }
}
