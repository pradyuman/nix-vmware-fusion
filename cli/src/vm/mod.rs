use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};

mod disk;
mod network;
mod optical;
mod plist;
mod schema;
mod shared_folder;
mod state;
mod vmcli;
mod vmx;

#[cfg(all(test, feature = "vmware-tests"))]
mod test_support;

// Inspect

struct Snapshot {
    vmx: vmx::Snapshot,
    plist: plist::Snapshot,
    network: network::Snapshot,
    disk: disk::Snapshot,
    shared_folder: shared_folder::Snapshot,
    optical: optical::Snapshot,
}

fn inspect(configured: &schema::VirtualMachine) -> Result<Snapshot> {
    let vmx = vmx::inspect(configured.path.as_ref())?;
    let plist = plist::inspect(&vmx.target_path)?;
    let network = network::inspect(&vmx.target_path)?;
    let disk = disk::inspect(&vmx.target_path, &configured.disks)?;
    let shared_folder = shared_folder::inspect(&vmx.target_path)?;
    let optical = optical::inspect(&vmx.target_path)?;

    Ok(Snapshot {
        vmx,
        plist,
        network,
        disk,
        shared_folder,
        optical,
    })
}

// Plan

struct Plan {
    vmx: vmx::Plan,
    plist: plist::Plan,
    network: network::Plan,
    disk: disk::Plan,
    shared_folder: shared_folder::Plan,
    optical: optical::Plan,
}

fn plan(configured: &schema::VirtualMachine, snapshot: Snapshot) -> Result<Plan> {
    let vmx = vmx::plan(configured, snapshot.vmx)?;
    let plist = plist::plan(configured.display.as_ref(), snapshot.plist);
    let network = network::plan(&configured.network_adapters, snapshot.network)?;
    let disk = disk::plan(&configured.disks, snapshot.disk)?;
    let shared_folder = shared_folder::plan(&configured.shared_folders, snapshot.shared_folder)?;
    let optical = optical::plan(&configured.optical_drives, snapshot.optical);

    Ok(Plan {
        vmx,
        plist,
        network,
        disk,
        shared_folder,
        optical,
    })
}

// Stage

struct StagedChange {
    vmx: vmx::StagedChange,
    plist: plist::StagedChange,
    disk: disk::StagedChange,
    optical: optical::StagedChange,
}

fn stage(plan: Plan) -> Result<StagedChange> {
    let Plan {
        vmx,
        plist,
        network,
        disk,
        shared_folder,
        optical,
    } = plan;
    let temp_dir = tempfile::tempdir()?;
    let filename = vmx
        .snapshot
        .target_path
        .file_name()
        .context("missing VMX filename")?;
    let draft_path = temp_dir.path().join(filename);

    vmx::stage(&draft_path, &vmx)?;
    network::stage(&draft_path, network)?;
    shared_folder::stage(&draft_path, shared_folder)?;

    let plist = plist::stage(plist)?;
    let disk = disk::stage(&draft_path, disk)?;
    let optical = optical::stage(&draft_path, optical)?;

    // Carry only the completed VMX forward and discard temporary baseline files
    let updated_contents = fs::read_to_string(&draft_path)?;

    Ok(StagedChange {
        vmx: vmx::StagedChange {
            snapshot: vmx.snapshot,
            updated_contents,
        },
        plist,
        disk,
        optical,
    })
}

// Commit

fn commit(staged: StagedChange) -> Result<()> {
    let vmx_changed = !staged.vmx.is_noop();
    let plist_changed = !staged.plist.is_noop();
    let disk_changed = !staged.disk.is_noop();

    let bundle_path = staged
        .vmx
        .snapshot
        .target_path
        .parent()
        .context("missing VMX directory")?
        .to_owned();

    if vmx_changed || plist_changed || disk_changed {
        ensure_stopped(&staged.vmx.snapshot.target_path)?;
    }
    staged.disk.commit()?;
    staged.vmx.commit()?;
    staged.plist.commit()?;

    // State can change without the VMX
    staged.optical.commit(&bundle_path)?;

    Ok(())
}

fn ensure_stopped(vmx_path: &Path) -> Result<()> {
    if !vmx_path.try_exists()? {
        return Ok(());
    }

    let bundle_path = vmx_path.parent().context("missing VMX directory")?;
    let state = vmcli::power_state(vmx_path)?;

    ensure!(
        state == "off",
        "{} must be fully shut down before applying changes (current power state: {state})",
        bundle_path.display()
    );

    Ok(())
}

// Apply

// Apply every configuration before failing so one virtual machine cannot block the others
pub(crate) fn apply_all(files: &[PathBuf]) -> Result<()> {
    let failures = files
        .iter()
        .filter_map(|file| {
            apply(file)
                .with_context(|| format!("could not apply {}", file.display()))
                .err()
        })
        .inspect(|error| eprintln!("Error: {error:#}"))
        .count();

    ensure!(
        failures == 0,
        "could not apply {failures} of {} virtual machine configurations",
        files.len()
    );

    Ok(())
}

fn apply(file: &Path) -> Result<()> {
    let contents = fs::read_to_string(file)?;
    let configured = serde_json::from_str::<schema::VirtualMachine>(&contents)?;

    let snapshot = inspect(&configured)?;
    let plan = plan(&configured, snapshot)?;
    let staged = stage(plan)?;

    commit(staged)
}

#[cfg(all(test, feature = "vmware-tests"))]
mod tests;
