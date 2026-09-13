use anyhow::Result;
use std::fs;
use std::path::Path;

mod schema;
mod vmx;

pub fn apply(file: &Path) -> Result<()> {
    let contents = fs::read_to_string(file)?;
    let schema = serde_json::from_str::<schema::VirtualMachine>(&contents)?;
    let bundle_path = schema.path.as_ref();

    let snapshot = vmx::inspect(bundle_path)?;
    let staged = vmx::stage(&schema, snapshot)?;

    if !staged.is_noop() {
        staged.commit()?;
    }

    Ok(())
}
