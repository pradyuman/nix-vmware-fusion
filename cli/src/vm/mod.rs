use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::config::CONFIG;

mod disks;
mod schema;
mod vmx;

#[cfg(all(test, feature = "vmware-tests"))]
mod test_support;

// Inspect

pub(crate) struct Snapshot {
    vmx: vmx::Snapshot,
    disks: disks::Snapshot,
}

fn inspect(schema: &schema::VirtualMachine) -> Result<Snapshot> {
    let vmx = vmx::inspect(schema.path.as_ref())?;
    let disks = disks::inspect(&vmx.target_path, &schema.disks)?;

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

#[cfg(all(test, feature = "vmware-tests"))]
mod tests {
    use std::path::Path;

    use super::disks::BYTES_PER_GIB;
    use super::schema::VirtualMachine;
    use super::test_support::{GUEST_OS, assert_vmx_entry, create_vmdk, create_vmx};
    use super::*;

    fn virtual_machine_ir(bundle_path: &Path, disk_path: &Path) -> serde_json::Value {
        serde_json::json!({
            "displayName": "Test VM",
            "path": bundle_path,
            "guestOS": GUEST_OS,
            "vcpus": 4,
            "memory": 4096,
            "secureBoot": true,
            "disks": {
                "primary": {
                    "path": disk_path,
                    "size": 1,
                    "bus": "nvme"
                }
            }
        })
    }

    fn write_ir_file(path: &Path, ir: &serde_json::Value) -> Result<()> {
        fs::write(path, serde_json::to_vec(ir)?)?;

        Ok(())
    }

    #[test]
    fn vmcli_reports_new_vmx_as_stopped() -> Result<()> {
        let (_temp_dir, vmx_path) = create_vmx()?;

        ensure_stopped(&vmx_path)?;

        Ok(())
    }

    #[test]
    fn applies_virtual_machine_configuration() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let bundle_path = temp_dir.path().join("test.vmwarevm");
        let vmx_path = bundle_path.join("test.vmx");
        let disk_path = temp_dir.path().join("managed.vmdk");
        let ir_path = temp_dir.path().join("virtual-machine-ir.json");

        let ir = virtual_machine_ir(&bundle_path, &disk_path);
        write_ir_file(&ir_path, &ir)?;

        // Apply the initial configuration
        apply(&ir_path)?;

        assert_vmx_entry(&vmx_path, "displayName", "Test VM")?;
        assert_vmx_entry(&vmx_path, "guestOS", GUEST_OS)?;
        assert_vmx_entry(&vmx_path, "numvcpus", "4")?;
        assert_vmx_entry(&vmx_path, "memsize", "4096")?;
        assert_vmx_entry(&vmx_path, "uefi.secureBoot.enabled", "TRUE")?;
        assert_vmx_entry(&vmx_path, "firmware", "efi")?;

        let schema = serde_json::from_value::<VirtualMachine>(ir)?;
        let snapshot = inspect(&schema)?;
        let attached = &snapshot.disks.attached_disks;

        assert_eq!(
            snapshot.disks.inspected_disks[0]
                .current_state
                .as_ref()
                .expect("existing disk state")
                .capacity_bytes
                .get(),
            BYTES_PER_GIB
        );
        assert_eq!(attached.len(), 1);
        assert!(attached[0].label.starts_with("nvme"));
        assert_eq!(
            attached[0].canonical_path,
            Some(fs::canonicalize(&disk_path)?)
        );

        // Reapplying the same IR should leave the VMX unchanged
        let vmx_contents = fs::read_to_string(&vmx_path)?;
        apply(&ir_path)?;
        assert_eq!(fs::read_to_string(&vmx_path)?, vmx_contents);

        Ok(())
    }

    #[test]
    fn updates_existing_virtual_machine_configuration() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let bundle_path = temp_dir.path().join("test.vmwarevm");
        let vmx_path = bundle_path.join("test.vmx");
        let disk_path = temp_dir.path().join("managed.vmdk");
        let ir_path = temp_dir.path().join("virtual-machine-ir.json");

        create_vmdk(&disk_path, "10MB")?;

        // Create the initial virtual machine
        let mut ir = virtual_machine_ir(&bundle_path, &disk_path);
        write_ir_file(&ir_path, &ir)?;
        apply(&ir_path)?;

        // Update its settings and move the disk to SATA
        let display_name = "Updated Test VM";
        let vcpus = 6;
        let memory = 8192;
        let secure_boot = false;
        let bus = "sata";

        ir["displayName"] = serde_json::json!(display_name);
        ir["vcpus"] = serde_json::json!(vcpus);
        ir["memory"] = serde_json::json!(memory);
        ir["secureBoot"] = serde_json::json!(secure_boot);
        ir["disks"]["primary"]["bus"] = serde_json::json!(bus);
        write_ir_file(&ir_path, &ir)?;

        // Apply the updated configuration
        apply(&ir_path)?;

        assert_vmx_entry(&vmx_path, "displayName", display_name)?;
        assert_vmx_entry(&vmx_path, "numvcpus", &vcpus.to_string())?;
        assert_vmx_entry(&vmx_path, "memsize", &memory.to_string())?;
        assert_vmx_entry(
            &vmx_path,
            "uefi.secureBoot.enabled",
            if secure_boot { "TRUE" } else { "FALSE" },
        )?;

        let schema = serde_json::from_value::<VirtualMachine>(ir)?;
        let attached = inspect(&schema)?.disks.attached_disks;

        assert_eq!(attached.len(), 1);
        assert!(attached[0].label.starts_with(bus));
        assert_eq!(
            attached[0].canonical_path,
            Some(fs::canonicalize(&disk_path)?)
        );

        Ok(())
    }
}
