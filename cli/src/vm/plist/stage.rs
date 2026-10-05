use ::plist::{Dictionary, Value};
use anyhow::{Context, Result};
use std::io::Cursor;

use super::{Action, Plan, StagedChange, StagedWrite};

const SCALED_HIGH_RESOLUTION_KEY: &str = "scaledHighResolution";

pub(crate) fn stage(plan: Plan) -> Result<StagedChange> {
    let mut document = match plan.snapshot.raw_contents.as_deref() {
        Some(contents) => Value::from_reader(Cursor::new(contents))?,
        None if matches!(&plan.action, Action::RemoveScaledHighResolution) => {
            return Ok(StagedChange { write: None });
        }
        None => Value::Dictionary(Dictionary::new()),
    };

    let dictionary = document
        .as_dictionary_mut()
        .context("virtual machine plist is not a dictionary")?;

    let changed = match plan.action {
        Action::SetScaledHighResolution(value) => {
            let existing = dictionary.get(SCALED_HIGH_RESOLUTION_KEY);
            let matches = existing
                .is_some_and(|existing| existing.as_unsigned_integer() == Some(u64::from(value)));

            if !matches {
                dictionary.insert(
                    SCALED_HIGH_RESOLUTION_KEY.to_owned(),
                    Value::from(i64::from(value)),
                );
            }

            !matches
        }
        Action::RemoveScaledHighResolution => {
            dictionary.remove(SCALED_HIGH_RESOLUTION_KEY).is_some()
        }
    };

    if !changed {
        return Ok(StagedChange { write: None });
    }

    let mut contents = Vec::new();
    document.to_writer_binary(&mut contents)?;

    Ok(StagedChange {
        write: Some(StagedWrite {
            path: plan.snapshot.target_path,
            contents,
        }),
    })
}

#[cfg(test)]
mod tests {
    use crate::vm::plist::Snapshot;

    use super::*;

    fn plan(raw_contents: Option<Vec<u8>>, action: Action) -> Plan {
        Plan {
            snapshot: Snapshot {
                target_path: "test.plist".into(),
                raw_contents,
            },
            action,
        }
    }

    fn dictionary(contents: &[u8]) -> Result<Dictionary> {
        Value::from_reader(Cursor::new(contents))?
            .into_dictionary()
            .context("test plist is not a dictionary")
    }

    #[test]
    fn scaled_high_resolution_is_reconciled_without_losing_other_entries() -> Result<()> {
        let mut baseline = Dictionary::new();
        baseline.insert("unrelated".to_owned(), Value::from("preserved"));

        let mut baseline_contents = Vec::new();
        Value::Dictionary(baseline).to_writer_binary(&mut baseline_contents)?;

        let staged = stage(plan(
            Some(baseline_contents),
            Action::SetScaledHighResolution(2),
        ))?
        .write
        .unwrap();
        let staged_contents = &staged.contents;
        let staged_dictionary = dictionary(staged_contents)?;

        assert_eq!(
            staged_dictionary
                .get(SCALED_HIGH_RESOLUTION_KEY)
                .and_then(Value::as_signed_integer),
            Some(2)
        );
        assert_eq!(
            staged_dictionary
                .get("unrelated")
                .and_then(Value::as_string),
            Some("preserved")
        );

        let reapplied = stage(plan(
            Some(staged_contents.clone()),
            Action::SetScaledHighResolution(2),
        ))?;

        assert!(reapplied.is_noop());

        let removed = stage(plan(
            Some(staged_contents.clone()),
            Action::RemoveScaledHighResolution,
        ))?
        .write
        .unwrap();
        let removed_dictionary = dictionary(&removed.contents)?;

        assert!(!removed_dictionary.contains_key(SCALED_HIGH_RESOLUTION_KEY));
        assert_eq!(
            removed_dictionary
                .get("unrelated")
                .and_then(Value::as_string),
            Some("preserved")
        );

        Ok(())
    }
}
