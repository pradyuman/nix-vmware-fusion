use anyhow::Result;

use crate::vm::schema::VirtualMachine;

use super::{Plan, Snapshot};

mod base;
mod cpu;
mod sound;

pub(crate) fn plan(schema: &VirtualMachine, snapshot: Snapshot) -> Result<Plan> {
    let actions = base::plan(schema, &snapshot)
        .into_iter()
        .chain(cpu::plan(schema)?)
        .chain(sound::plan(schema.sound.as_ref()))
        .collect();

    Ok(Plan {
        snapshot,
        guest_os: schema.guest_os.clone(),
        actions,
    })
}
