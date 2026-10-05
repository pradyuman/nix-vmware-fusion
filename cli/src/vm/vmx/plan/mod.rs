use anyhow::Result;

use crate::vm::schema::VirtualMachine;

use super::{Plan, Snapshot};

mod base;
mod cpu;
mod display;
mod sound;
mod usb;

pub(crate) fn plan(configured: &VirtualMachine, snapshot: Snapshot) -> Result<Plan> {
    let actions = base::plan(configured, &snapshot)
        .into_iter()
        .chain(cpu::plan(configured)?)
        .chain(display::plan(configured.display.as_ref()))
        .chain(sound::plan(configured.sound.as_ref()))
        .chain(usb::plan(configured.usb.as_ref()))
        .collect();

    Ok(Plan {
        snapshot,
        guest_os: configured.guest_os.clone(),
        actions,
    })
}
