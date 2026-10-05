use anyhow::Result;

use crate::vm::schema::VirtualMachine;

use super::{Action, Plan, Snapshot};

mod base;
mod cpu;
mod display;
mod isolation;
mod sound;
mod usb;

pub(crate) fn plan(configured: &VirtualMachine, snapshot: Snapshot) -> Result<Plan> {
    let actions = base::plan(configured, &snapshot)
        .into_iter()
        .chain(cpu::plan(configured)?)
        .chain(display::plan(configured.display.as_ref()))
        .chain(sound::plan(configured.sound.as_ref()))
        .chain(usb::plan(configured.usb.as_ref()))
        .chain(isolation::plan(&configured.isolation))
        .collect();

    Ok(Plan {
        snapshot,
        guest_os: configured.guest_os.clone(),
        actions,
    })
}

fn boolean(key: &'static str, value: bool, default: bool) -> Action {
    if value == default {
        Action::Remove(key)
    } else {
        Action::Set(key, if value { "TRUE" } else { "FALSE" }.to_owned())
    }
}

fn optional(key: &'static str, value: Option<&str>) -> Action {
    value.map_or_else(
        || Action::Remove(key),
        |value| Action::Set(key, value.to_owned()),
    )
}
