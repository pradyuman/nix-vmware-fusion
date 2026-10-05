use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::config::CONFIG;

mod disk;
mod network;
mod optical;
mod plist;
mod schema;
mod shared_folder;
mod state;
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

    // State can change without the VMX.
    staged.optical.commit(&bundle_path)?;

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
    let configured = serde_json::from_str::<schema::VirtualMachine>(&contents)?;

    let snapshot = inspect(&configured)?;
    let plan = plan(&configured, snapshot)?;
    let staged = stage(plan)?;

    commit(staged)
}

#[cfg(all(test, feature = "vmware-tests"))]
mod tests {
    use std::path::Path;

    use super::disk::BYTES_PER_GIB;
    use super::schema::VirtualMachine;
    use super::test_support::{GUEST_OS, assert, create_vmdk, create_vmx};
    use super::*;

    fn virtual_machine_ir(bundle_path: &Path, disk_path: &Path) -> serde_json::Value {
        serde_json::json!({
            "displayName": "Test VM",
            "path": bundle_path,
            "guestOS": GUEST_OS,
            "vcpus": 4,
            "coresPerSocket": 2,
            "memory": 4096,
            "secureBoot": true,
            "networkAdapters": {
                "primary": {}
            },
            "disks": {
                "primary": {
                    "path": disk_path,
                    "size": 1,
                    "bus": "nvme"
                }
            },
            "usb": {}
        })
    }

    fn write_ir_file(path: &Path, ir: &serde_json::Value) -> Result<()> {
        fs::write(path, serde_json::to_vec(ir)?)?;

        Ok(())
    }

    fn inspect_ir(ir: &serde_json::Value) -> Result<Snapshot> {
        let configured = serde_json::from_value::<VirtualMachine>(ir.clone())?;

        inspect(&configured)
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

        assert::vmx_entry(&vmx_path, "displayName", "Test VM")?;
        assert::vmx_entry(&vmx_path, "guestOS", GUEST_OS)?;
        assert::vmx_entry(&vmx_path, "numvcpus", "4")?;
        assert::vmx_entry(&vmx_path, "cpuid.coresPerSocket", "2")?;
        assert::vmx_entry(&vmx_path, "memsize", "4096")?;
        assert::vmx_entry(&vmx_path, "uefi.secureBoot.enabled", "TRUE")?;
        assert::vmx_entry(&vmx_path, "firmware", "efi")?;
        assert::vmx_entry(&vmx_path, "usb.present", "TRUE")?;
        assert::vmx_entry(&vmx_path, "ehci.present", "TRUE")?;
        assert::vmx_entry(&vmx_path, "usb_xhci.present", "TRUE")?;

        let snapshot = inspect_ir(&ir)?;
        let network_attachments = &snapshot.network.network_attachments;
        let disk_attachments = &snapshot.disk.disk_attachments;

        assert_eq!(network_attachments.len(), 1);
        assert_eq!(network_attachments[0].label, "ethernet0");
        assert_eq!(
            network_attachments[0].external_id,
            "nix-vmware-fusion:primary"
        );
        assert_eq!(network_attachments[0].mode, "nat");

        assert_eq!(
            snapshot.disk.disk_images[0].state.capacity_bytes.get(),
            BYTES_PER_GIB
        );
        assert_eq!(disk_attachments.len(), 1);
        assert!(disk_attachments[0].label.as_ref().starts_with("nvme"));
        assert_eq!(
            disk_attachments[0].canonical_path,
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
        ir["coresPerSocket"] = serde_json::Value::Null;
        ir["memory"] = serde_json::json!(memory);
        ir["secureBoot"] = serde_json::json!(secure_boot);
        ir["disks"]["primary"]["bus"] = serde_json::json!(bus);
        write_ir_file(&ir_path, &ir)?;

        // Apply the updated configuration
        apply(&ir_path)?;

        assert::vmx_entry(&vmx_path, "displayName", display_name)?;
        assert::vmx_entry(&vmx_path, "numvcpus", &vcpus.to_string())?;
        assert::vmx_entry_absent(&vmx_path, "cpuid.coresPerSocket")?;
        assert::vmx_entry(&vmx_path, "memsize", &memory.to_string())?;
        assert::vmx_entry(
            &vmx_path,
            "uefi.secureBoot.enabled",
            if secure_boot { "TRUE" } else { "FALSE" },
        )?;

        let snapshot = inspect_ir(&ir)?;
        let disk_attachments = &snapshot.disk.disk_attachments;

        assert_eq!(disk_attachments.len(), 1);
        assert!(disk_attachments[0].label.as_ref().starts_with(bus));
        assert_eq!(
            disk_attachments[0].canonical_path,
            Some(fs::canonicalize(&disk_path)?)
        );

        // Reapplying automatic topology should leave the VMX unchanged
        let vmx_contents = fs::read_to_string(&vmx_path)?;
        apply(&ir_path)?;
        assert_eq!(fs::read_to_string(&vmx_path)?, vmx_contents);

        Ok(())
    }

    #[test]
    fn manages_display_configuration() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let bundle_path = temp_dir.path().join("test.vmwarevm");
        let vmx_path = bundle_path.join("test.vmx");
        let plist_path = bundle_path.join("test.plist");
        let disk_path = temp_dir.path().join("managed.vmdk");
        let ir_path = temp_dir.path().join("virtual-machine-ir.json");

        // Apply configured display settings
        let mut ir = virtual_machine_ir(&bundle_path, &disk_path);
        ir["display"] = serde_json::json!({
            "graphics": {
                "accelerate3D": false,
                "memory": 515
            },
            "nativeDisplayResolution": {
                "enable": true,
                "scaledHighResolution": "single-window"
            },
            "singleWindowFit": "stretch",
            "fullScreenFit": "center",
            "useAllDisplaysInFullScreen": true
        });

        write_ir_file(&ir_path, &ir)?;
        apply(&ir_path)?;

        assert::vmx_entry_absent(&vmx_path, "mks.enable3d")?;
        assert::vmx_entry(&vmx_path, "svga.graphicsMemoryKB", "527360")?;
        assert::vmx_entry(&vmx_path, "gui.perVMWindowAutofitMode", "stretch")?;
        assert::vmx_entry(&vmx_path, "gui.perVMFullscreenAutofitMode", "center")?;
        assert::vmx_entry(&vmx_path, "gui.fullScreenOnAllHostDisplays", "TRUE")?;
        assert::vmx_entry(
            &vmx_path,
            "gui.fitGuestUsingNativeDisplayResolution",
            "TRUE",
        )?;
        assert::plist_integer(&plist_path, "scaledHighResolution", 2)?;

        // Apply another scaled high-resolution mode
        ir["display"]["nativeDisplayResolution"]["scaledHighResolution"] =
            serde_json::json!("full-screen");
        write_ir_file(&ir_path, &ir)?;
        apply(&ir_path)?;

        assert::plist_integer(&plist_path, "scaledHighResolution", 1)?;

        // Apply the default display settings
        ir["display"] = serde_json::json!({});
        write_ir_file(&ir_path, &ir)?;
        apply(&ir_path)?;

        assert::vmx_entry(&vmx_path, "mks.enable3d", "TRUE")?;
        assert::vmx_entry(&vmx_path, "svga.graphicsMemoryKB", "262144")?;
        [
            "gui.perVMWindowAutofitMode",
            "gui.perVMFullscreenAutofitMode",
            "gui.fullScreenOnAllHostDisplays",
            "gui.fitGuestUsingNativeDisplayResolution",
        ]
        .into_iter()
        .try_for_each(|key| assert::vmx_entry_absent(&vmx_path, key))?;
        assert::plist_entry_absent(&plist_path, "scaledHighResolution")?;

        // Remove the display configuration
        ir["display"] = serde_json::Value::Null;
        write_ir_file(&ir_path, &ir)?;
        apply(&ir_path)?;

        [
            "mks.enable3d",
            "svga.graphicsMemoryKB",
            "gui.perVMWindowAutofitMode",
            "gui.perVMFullscreenAutofitMode",
            "gui.fullScreenOnAllHostDisplays",
            "gui.fitGuestUsingNativeDisplayResolution",
        ]
        .into_iter()
        .try_for_each(|key| assert::vmx_entry_absent(&vmx_path, key))?;
        assert::plist_entry_absent(&plist_path, "scaledHighResolution")?;

        // Reapplying the same configuration should leave both files unchanged
        let vmx_contents = fs::read_to_string(&vmx_path)?;
        let plist_contents = fs::read(&plist_path)?;
        apply(&ir_path)?;
        assert_eq!(fs::read_to_string(&vmx_path)?, vmx_contents);
        assert_eq!(fs::read(&plist_path)?, plist_contents);

        Ok(())
    }

    #[test]
    fn manages_sound_card() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let bundle_path = temp_dir.path().join("test.vmwarevm");
        let vmx_path = bundle_path.join("test.vmx");
        let disk_path = temp_dir.path().join("managed.vmdk");
        let ir_path = temp_dir.path().join("virtual-machine-ir.json");

        // Apply the default sound card settings
        let mut ir = virtual_machine_ir(&bundle_path, &disk_path);
        ir["sound"] = serde_json::json!({});

        write_ir_file(&ir_path, &ir)?;
        apply(&ir_path)?;

        assert::vmx_entry(&vmx_path, "sound.present", "TRUE")?;
        assert::vmx_entry(&vmx_path, "sound.virtualDev", "hdaudio")?;
        assert::vmx_entry(&vmx_path, "sound.autoDetect", "TRUE")?;
        assert::vmx_entry(&vmx_path, "sound.fileName", "-1")?;
        assert::vmx_entry_absent(&vmx_path, "sound.startConnected")?;
        assert::vmx_entry_absent(&vmx_path, "sound.enableAEC")?;

        // Apply configured sound card settings
        ir["sound"]["startConnected"] = serde_json::json!(false);
        ir["sound"]["echoCancellation"] = serde_json::json!(true);
        write_ir_file(&ir_path, &ir)?;
        apply(&ir_path)?;

        assert::vmx_entry(&vmx_path, "sound.startConnected", "FALSE")?;
        assert::vmx_entry(&vmx_path, "sound.enableAEC", "TRUE")?;

        // Simulate the PCI slot number VMware Fusion assigns to a sound card
        duct::cmd!(
            &CONFIG.dict_tool,
            "-q",
            "set",
            &vmx_path,
            "sound.pciSlotNumber=515"
        )
        .run()?;

        // Remove the sound card
        ir["sound"] = serde_json::Value::Null;
        write_ir_file(&ir_path, &ir)?;
        apply(&ir_path)?;

        [
            "sound.present",
            "sound.virtualDev",
            "sound.autoDetect",
            "sound.fileName",
            "sound.startConnected",
            "sound.enableAEC",
            "sound.pciSlotNumber",
        ]
        .into_iter()
        .try_for_each(|key| assert::vmx_entry_absent(&vmx_path, key))?;

        // Reapplying the same configuration should leave the VMX unchanged
        let vmx_contents = fs::read_to_string(&vmx_path)?;
        apply(&ir_path)?;
        assert_eq!(fs::read_to_string(&vmx_path)?, vmx_contents);

        Ok(())
    }

    #[test]
    fn manages_usb_controller() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let bundle_path = temp_dir.path().join("test.vmwarevm");
        let vmx_path = bundle_path.join("test.vmx");
        let disk_path = temp_dir.path().join("managed.vmdk");
        let ir_path = temp_dir.path().join("virtual-machine-ir.json");

        // Apply the default USB controller configuration
        let mut ir = virtual_machine_ir(&bundle_path, &disk_path);
        write_ir_file(&ir_path, &ir)?;
        apply(&ir_path)?;

        assert::vmx_entry(&vmx_path, "usb.present", "TRUE")?;
        assert::vmx_entry(&vmx_path, "ehci.present", "TRUE")?;
        assert::vmx_entry(&vmx_path, "usb_xhci.present", "TRUE")?;
        assert::vmx_entry_absent(&vmx_path, "usb.generic.pluginAction")?;

        // Connect new USB devices to the virtual machine
        ir["usb"]["newDeviceAction"] = serde_json::json!("connect-to-vm");
        write_ir_file(&ir_path, &ir)?;
        apply(&ir_path)?;

        assert::vmx_entry(&vmx_path, "usb.generic.pluginAction", "guest")?;

        // Keep new USB devices connected to the host
        ir["usb"]["newDeviceAction"] = serde_json::json!("connect-to-host");
        write_ir_file(&ir_path, &ir)?;
        apply(&ir_path)?;

        assert::vmx_entry(&vmx_path, "usb.generic.pluginAction", "host")?;

        // Simulate the PCI slot number VMware Fusion assigns to the controller
        duct::cmd!(
            &CONFIG.dict_tool,
            "-q",
            "set",
            &vmx_path,
            "usb_xhci.pciSlotNumber=515"
        )
        .run()?;

        // Remove the USB controller without removing VMware-owned entries
        ir["usb"] = serde_json::Value::Null;
        write_ir_file(&ir_path, &ir)?;
        apply(&ir_path)?;

        assert::vmx_entry_absent(&vmx_path, "usb.present")?;
        assert::vmx_entry_absent(&vmx_path, "ehci.present")?;
        assert::vmx_entry_absent(&vmx_path, "usb_xhci.present")?;
        assert::vmx_entry_absent(&vmx_path, "usb.generic.pluginAction")?;
        assert::vmx_entry(&vmx_path, "usb_xhci.pciSlotNumber", "515")?;

        // Restore the USB controller
        ir["usb"] = serde_json::json!({});
        write_ir_file(&ir_path, &ir)?;
        apply(&ir_path)?;

        assert::vmx_entry(&vmx_path, "usb.present", "TRUE")?;
        assert::vmx_entry(&vmx_path, "ehci.present", "TRUE")?;
        assert::vmx_entry(&vmx_path, "usb_xhci.present", "TRUE")?;
        assert::vmx_entry_absent(&vmx_path, "usb.generic.pluginAction")?;
        assert::vmx_entry(&vmx_path, "usb_xhci.pciSlotNumber", "515")?;

        // Reapplying the same configuration should leave the VMX unchanged
        let vmx_contents = fs::read_to_string(&vmx_path)?;
        apply(&ir_path)?;
        assert_eq!(fs::read_to_string(&vmx_path)?, vmx_contents);

        Ok(())
    }

    #[test]
    fn manages_optical_drive_lifecycle() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let bundle_path = temp_dir.path().join("test.vmwarevm");
        let image_path = temp_dir.path().join("installer.iso");
        let updated_image_path = temp_dir.path().join("updated-installer.iso");
        let ir_path = temp_dir.path().join("virtual-machine-ir.json");

        fs::write(&image_path, [])?;
        fs::write(&updated_image_path, [])?;

        let mut ir = serde_json::json!({
            "displayName": "Test VM",
            "path": bundle_path,
            "guestOS": GUEST_OS,
            "vcpus": 2,
            "memory": 4096,
            "secureBoot": false,
            "opticalDrives": {
                "installer": {
                    "source": {
                        "type": "image",
                        "path": image_path
                    }
                }
            }
        });

        // Create the optical drive
        write_ir_file(&ir_path, &ir)?;
        apply(&ir_path)?;

        let snapshot = inspect_ir(&ir)?;
        let [attachment] = snapshot.optical.optical_attachments.as_slice() else {
            panic!("expected one optical attachment");
        };
        let label = attachment.label.clone();

        assert_eq!(
            attachment.backing_path.as_deref(),
            Some(image_path.as_path())
        );
        assert!(attachment.start_connected);
        assert_eq!(
            snapshot
                .optical
                .state
                .optical_drives
                .get("installer")
                .expect("managed installer drive")
                .label,
            label
        );

        // Replace the image while retaining the assigned VMware label
        ir["opticalDrives"]["installer"]["source"]["path"] = serde_json::json!(updated_image_path);
        ir["opticalDrives"]["installer"]["startConnected"] = serde_json::json!(false);
        write_ir_file(&ir_path, &ir)?;
        apply(&ir_path)?;

        let snapshot = inspect_ir(&ir)?;
        let [attachment] = snapshot.optical.optical_attachments.as_slice() else {
            panic!("expected one optical attachment");
        };

        assert_eq!(attachment.label, label);
        assert_eq!(
            attachment.backing_path.as_deref(),
            Some(updated_image_path.as_path())
        );
        assert!(!attachment.start_connected);

        // Remove the managed optical drive
        ir["opticalDrives"] = serde_json::json!({});
        write_ir_file(&ir_path, &ir)?;
        apply(&ir_path)?;

        let snapshot = inspect_ir(&ir)?;

        assert!(snapshot.optical.optical_attachments.is_empty());
        assert!(snapshot.optical.state.optical_drives.is_empty());

        Ok(())
    }

    #[test]
    fn renamed_bundle_uses_existing_vmx_and_plist_filenames() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let original_bundle_path = temp_dir.path().join("original.vmwarevm");
        let renamed_bundle_path = temp_dir.path().join("renamed.vmwarevm");
        let disk_path = temp_dir.path().join("managed.vmdk");
        let ir_path = temp_dir.path().join("virtual-machine-ir.json");

        // Create the VM using its original bundle name
        let mut ir = virtual_machine_ir(&original_bundle_path, &disk_path);
        ir["display"] = serde_json::json!({
            "nativeDisplayResolution": {
                "enable": true,
                "scaledHighResolution": "single-window"
            }
        });
        write_ir_file(&ir_path, &ir)?;
        apply(&ir_path)?;

        // Rename only the bundle, leaving its internal filenames unchanged
        fs::rename(&original_bundle_path, &renamed_bundle_path)?;
        ir["path"] = serde_json::json!(renamed_bundle_path);
        ir["displayName"] = serde_json::json!("Renamed VM");
        ir["display"]["nativeDisplayResolution"]["scaledHighResolution"] =
            serde_json::json!("full-screen");
        write_ir_file(&ir_path, &ir)?;
        apply(&ir_path)?;

        let vmx_path = renamed_bundle_path.join("original.vmx");
        let plist_path = renamed_bundle_path.join("original.plist");

        assert::vmx_entry(&vmx_path, "displayName", "Renamed VM")?;
        assert::plist_integer(&plist_path, "scaledHighResolution", 1)?;
        assert!(!renamed_bundle_path.join("renamed.vmx").try_exists()?);
        assert!(!renamed_bundle_path.join("renamed.plist").try_exists()?);

        Ok(())
    }
}
