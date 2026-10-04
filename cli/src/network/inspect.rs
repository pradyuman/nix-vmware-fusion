use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use super::state::State;
use super::{NetworkAnswers, Snapshot};

const NETWORKING_PATH: &str = "/Library/Preferences/VMware Fusion/networking";

pub(super) fn inspect() -> Result<Snapshot> {
    Ok(Snapshot {
        state: State::load()?,
        answers: read_answers(Path::new(NETWORKING_PATH))?,
    })
}

fn read_answers(path: &Path) -> Result<NetworkAnswers> {
    let contents = fs::read_to_string(path).with_context(|| {
        format!(
            "could not read VMware Fusion networking file {}",
            path.display()
        )
    })?;

    Ok(contents
        .lines()
        .filter_map(|line| line.strip_prefix("answer "))
        .filter_map(|line| line.split_once(' '))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect())
}
