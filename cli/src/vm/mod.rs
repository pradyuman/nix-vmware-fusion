use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::config::CONFIG;

mod disks;
mod schema;
mod vmx;

// Inspect

pub(crate) struct Snapshot {
    vmx: vmx::Snapshot,
    disks: disks::Snapshot,
}

fn inspect(schema: &schema::VirtualMachine) -> Result<Snapshot> {
    let vmx = vmx::inspect(schema.path.as_ref())?;
    let disks = disks::inspect(&schema.disks, &vmx.target_path)?;

    Ok(Snapshot { vmx, disks })
}

// Plan

pub(crate) struct Plan {
    vmx: vmx::Plan,
    disks: disks::Plan,
}

fn plan(schema: &schema::VirtualMachine, snapshot: Snapshot) -> Result<Plan> {
    let disks = disks::plan(snapshot.disks)?;
    let vmx = vmx::plan(schema, snapshot.vmx);

    Ok(Plan { vmx, disks })
}

// Stage

pub(crate) struct StagedChange {
    vmx: vmx::StagedChange,
    disks: disks::StagedChange,
}

fn stage(plan: Plan) -> Result<StagedChange> {
    let Plan { vmx, disks } = plan;
    let temp_dir = tempfile::tempdir()?;
    let filename = vmx
        .snapshot
        .target_path
        .file_name()
        .context("missing VMX filename")?;
    let draft_path = temp_dir.path().join(filename);

    vmx::stage(&draft_path, &vmx)?;
    let disks = disks::stage(&draft_path, disks)?;

    // Carry only the completed VMX forward and discard temporary baseline files
    let updated_contents = fs::read_to_string(&draft_path)?;

    Ok(StagedChange {
        vmx: vmx::StagedChange {
            snapshot: vmx.snapshot,
            updated_contents,
        },
        disks,
    })
}

// Commit

fn commit(staged: StagedChange) -> Result<()> {
    let vmx_changed = !staged.vmx.is_noop();
    let disks_changed = !staged.disks.is_noop();

    if vmx_changed || disks_changed {
        ensure_stopped(&staged.vmx.snapshot.target_path)?;
    }
    if disks_changed {
        staged.disks.commit()?;
    }
    if vmx_changed {
        staged.vmx.commit()?;
    }

    Ok(())
}

fn ensure_stopped(vmx_path: &Path) -> Result<()> {
    if !vmx_path.try_exists()? {
        return Ok(());
    }

    let bundle_path = vmx_path.parent().context("missing VMX directory")?;
    let power = duct::cmd!(&CONFIG.vmcli, vmx_path, "power", "query")
        .read()
        .context("could not query virtual machine power state")?;

    let state = power
        .lines()
        .find_map(|line| line.strip_prefix("PowerState:"))
        .map(str::trim)
        .context("vmcli did not report a power state")?;

    anyhow::ensure!(
        state == "off",
        "{} must be fully shut down before applying changes (current power state: {state})",
        bundle_path.display()
    );

    Ok(())
}

// Apply

pub(crate) fn apply(file: &Path) -> Result<()> {
    let contents = fs::read_to_string(file)?;
    let schema = serde_json::from_str::<schema::VirtualMachine>(&contents)?;

    let snapshot = inspect(&schema)?;
    let plan = plan(&schema, snapshot)?;
    let staged = stage(plan)?;

    commit(staged)
}
