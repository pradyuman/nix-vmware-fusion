use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StateFile<T> {
    format_version: u64,
    #[serde(flatten)]
    state: T,
}

pub(crate) trait PersistentState:
    Default + DeserializeOwned + PartialEq + Serialize
{
    const FORMAT_VERSION: u64;

    fn load_from(path: &Path) -> Result<Self> {
        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "could not read nix-vmware-fusion state file {}",
                        path.display()
                    )
                });
            }
        };

        let state_file = serde_json::from_str::<StateFile<Self>>(&contents).with_context(|| {
            format!(
                "could not parse nix-vmware-fusion state file {}",
                path.display()
            )
        })?;

        ensure!(
            state_file.format_version == Self::FORMAT_VERSION,
            "unsupported nix-vmware-fusion state format version {} in {}",
            state_file.format_version,
            path.display()
        );

        Ok(state_file.state)
    }

    fn save_to(&self, path: &Path) -> Result<()> {
        if Self::load_from(path)? == *self {
            return Ok(());
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "could not create nix-vmware-fusion state directory {}",
                    parent.display()
                )
            })?;
        }

        let mut file = AtomicWriteFile::open(path).with_context(|| {
            format!(
                "could not open nix-vmware-fusion state file {}",
                path.display()
            )
        })?;

        let state_file = StateFile {
            format_version: Self::FORMAT_VERSION,
            state: self,
        };
        serde_json::to_writer_pretty(&mut file, &state_file).with_context(|| {
            format!(
                "could not serialize nix-vmware-fusion state to {}",
                path.display()
            )
        })?;
        file.write_all(b"\n")?;
        file.commit().with_context(|| {
            format!(
                "could not save nix-vmware-fusion state file {}",
                path.display()
            )
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default, Deserialize, PartialEq, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct TestState {
        #[serde(default)]
        value: Option<String>,
    }

    impl PersistentState for TestState {
        const FORMAT_VERSION: u64 = 1;
    }

    #[test]
    fn missing_state_uses_default() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let state = TestState::load_from(&temp_dir.path().join("state.json"))?;

        assert_eq!(state, TestState::default());

        Ok(())
    }

    #[test]
    fn unsupported_format_version_is_rejected() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let path = temp_dir.path().join("state.json");
        fs::write(&path, r#"{"formatVersion": 2}"#)?;

        let error = TestState::load_from(&path).expect_err("unsupported version");

        assert!(
            error
                .to_string()
                .contains("unsupported nix-vmware-fusion state format version 2")
        );

        Ok(())
    }
}
