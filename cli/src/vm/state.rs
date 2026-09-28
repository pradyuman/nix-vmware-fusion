use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};

use super::optical::OpticalDriveLabel;

const FILENAME: &str = ".nix-vmware-fusion.json";
const FORMAT_VERSION: u64 = 1;

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct State {
    format_version: u64,
    #[serde(default)]
    pub optical_drives: BTreeMap<String, OpticalDriveBinding>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct OpticalDriveBinding {
    pub label: OpticalDriveLabel,
}

impl Default for State {
    fn default() -> Self {
        Self {
            format_version: FORMAT_VERSION,
            optical_drives: BTreeMap::new(),
        }
    }
}

impl State {
    pub(crate) fn load(bundle_path: &Path) -> Result<Self> {
        let path = bundle_path.join(FILENAME);
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("could not read state from {}", path.display()));
            }
        };

        let state = serde_json::from_str::<Self>(&contents)
            .with_context(|| format!("could not parse state from {}", path.display()))?;

        ensure!(
            state.format_version == FORMAT_VERSION,
            "unsupported state format version {} in {}",
            state.format_version,
            path.display()
        );

        Ok(state)
    }

    pub(crate) fn save(&self, bundle_path: &Path) -> Result<()> {
        if Self::load(bundle_path)? == *self {
            return Ok(());
        }

        fs::create_dir_all(bundle_path)
            .with_context(|| format!("could not create VM bundle {}", bundle_path.display()))?;

        let path = bundle_path.join(FILENAME);
        let mut file = AtomicWriteFile::open(&path)
            .with_context(|| format!("could not open state file {}", path.display()))?;

        serde_json::to_writer_pretty(&mut file, self)
            .with_context(|| format!("could not serialize state to {}", path.display()))?;
        file.write_all(b"\n")?;
        file.commit()
            .with_context(|| format!("could not save state to {}", path.display()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_state_is_empty() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let state = State::load(temp_dir.path())?;

        assert!(state.optical_drives.is_empty());

        Ok(())
    }

    #[test]
    fn unchanged_empty_state_is_not_saved() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;

        State::default().save(temp_dir.path())?;

        assert!(!temp_dir.path().join(FILENAME).exists());

        Ok(())
    }

    #[test]
    fn optical_drive_labels_are_loaded() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        fs::write(
            temp_dir.path().join(FILENAME),
            r#"{
                "formatVersion": 1,
                "opticalDrives": {
                    "installer": {
                        "label": "sata0:1"
                    }
                }
            }"#,
        )?;

        let state = State::load(temp_dir.path())?;

        assert_eq!(state.optical_drives["installer"].label, "sata0:1");

        Ok(())
    }

    #[test]
    fn state_is_saved_and_loaded() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let mut state = State::default();
        state.optical_drives.insert(
            "installer".to_owned(),
            OpticalDriveBinding {
                label: "sata0:1".to_owned(),
            },
        );

        state.save(temp_dir.path())?;

        let state = State::load(temp_dir.path())?;

        assert_eq!(state.format_version, FORMAT_VERSION);
        assert_eq!(state.optical_drives["installer"].label, "sata0:1");

        Ok(())
    }

    #[test]
    fn unsupported_format_version_is_rejected() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        fs::write(temp_dir.path().join(FILENAME), r#"{"formatVersion": 2}"#)?;

        let error = State::load(temp_dir.path()).expect_err("unsupported version");

        assert!(
            error
                .to_string()
                .contains("unsupported state format version 2")
        );

        Ok(())
    }
}
