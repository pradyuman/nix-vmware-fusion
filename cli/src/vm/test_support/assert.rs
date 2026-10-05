use anyhow::Result;
use std::path::Path;

use crate::config::CONFIG;

pub(crate) fn vmx_entry(path: &Path, key: &str, value: &str) -> Result<()> {
    let entry = duct::cmd!(&CONFIG.dict_tool, "-q", "query", path, key).read()?;

    assert_eq!(entry, format!(r#"{key} = "{value}""#));

    Ok(())
}

pub(crate) fn vmx_entry_absent(path: &Path, key: &str) -> Result<()> {
    let output = duct::cmd!(&CONFIG.dict_tool, "-q", "query", path, key)
        .stderr_null()
        .unchecked()
        .run()?;

    assert!(!output.status.success(), "VMX entry {key} is present");

    Ok(())
}

pub(crate) fn plist_integer(path: &Path, key: &str, value: u8) -> Result<()> {
    let plist = plist::Value::from_file(path)?;
    let entry = plist
        .as_dictionary()
        .and_then(|dictionary| dictionary.get(key))
        .and_then(plist::Value::as_unsigned_integer);

    assert_eq!(entry, Some(u64::from(value)));

    Ok(())
}

pub(crate) fn plist_entry_absent(path: &Path, key: &str) -> Result<()> {
    let plist = plist::Value::from_file(path)?;
    let present = plist
        .as_dictionary()
        .is_some_and(|dictionary| dictionary.contains_key(key));

    assert!(!present, "plist entry {key} is present");

    Ok(())
}
