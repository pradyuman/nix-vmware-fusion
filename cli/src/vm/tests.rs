use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::*;
use crate::vm::disk::BYTES_PER_GIB;
use crate::vm::schema::VirtualMachine;
use crate::vm::test_support::{GUEST_OS, assert, create_vmdk, create_vmx};

// A virtual machine bundle in a temporary directory, configured through an IR file
struct Fixture {
    temp_dir: TempDir,
    bundle_path: PathBuf,
    vmx_path: PathBuf,
    plist_path: PathBuf,
    disk_path: PathBuf,
    ir_path: PathBuf,
    ir: serde_json::Value,
}

impl Fixture {
    fn new() -> Result<Self> {
        Self::named("test")
    }

    // VMware Fusion names the VMX and plist after the bundle when it creates them
    fn named(name: &str) -> Result<Self> {
        let temp_dir = tempfile::tempdir()?;
        let bundle_path = temp_dir.path().join(format!("{name}.vmwarevm"));
        let disk_path = temp_dir.path().join("managed.vmdk");

        Ok(Self {
            vmx_path: bundle_path.join(format!("{name}.vmx")),
            plist_path: bundle_path.join(format!("{name}.plist")),
            ir_path: temp_dir.path().join("virtual-machine-ir.json"),
            ir: virtual_machine_ir(&bundle_path, &disk_path),
            bundle_path,
            disk_path,
            temp_dir,
        })
    }

    fn apply(&self) -> Result<()> {
        fs::write(&self.ir_path, serde_json::to_vec(&self.ir)?)?;

        super::apply(&self.ir_path)
    }

    fn inspect(&self) -> Result<Snapshot> {
        let configured = serde_json::from_value::<VirtualMachine>(self.ir.clone())?;

        super::inspect(&configured)
    }

    // Reapply the current IR and check that neither the VMX nor the plist changes
    fn assert_reapply_unchanged(&self) -> Result<()> {
        let read_plist = || {
            self.plist_path
                .try_exists()?
                .then(|| fs::read(&self.plist_path))
                .transpose()
        };

        let vmx_contents = fs::read(&self.vmx_path)?;
        let plist_contents = read_plist()?;
        self.apply()?;

        assert_eq!(fs::read(&self.vmx_path)?, vmx_contents);
        assert_eq!(read_plist()?, plist_contents);

        Ok(())
    }
}

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

#[test]
fn vmcli_reports_new_vmx_as_stopped() -> Result<()> {
    let (_temp_dir, vmx_path) = create_vmx()?;

    ensure_stopped(&vmx_path)?;

    Ok(())
}

#[test]
fn applies_virtual_machine_configuration() -> Result<()> {
    let fixture = Fixture::new()?;
    let vmx_path = &fixture.vmx_path;

    // Apply the initial configuration
    fixture.apply()?;

    assert::vmx_entry(vmx_path, "displayName", "Test VM")?;
    assert::vmx_entry(vmx_path, "guestOS", GUEST_OS)?;
    assert::vmx_entry(vmx_path, "numvcpus", "4")?;
    assert::vmx_entry(vmx_path, "cpuid.coresPerSocket", "2")?;
    assert::vmx_entry(vmx_path, "memsize", "4096")?;
    assert::vmx_entry(vmx_path, "uefi.secureBoot.enabled", "TRUE")?;
    assert::vmx_entry(vmx_path, "firmware", "efi")?;
    assert::vmx_entry(vmx_path, "usb.present", "TRUE")?;
    assert::vmx_entry(vmx_path, "ehci.present", "TRUE")?;
    assert::vmx_entry(vmx_path, "usb_xhci.present", "TRUE")?;

    let snapshot = fixture.inspect()?;
    let network_attachments = &snapshot.network.network_attachments;
    let disk_attachments = &snapshot.disk.disk_attachments;

    assert_eq!(network_attachments.len(), 1);
    assert_eq!(network_attachments[0].label.as_ref(), "ethernet0");
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
        Some(fs::canonicalize(&fixture.disk_path)?)
    );

    fixture.assert_reapply_unchanged()?;

    Ok(())
}

#[test]
fn updates_existing_virtual_machine_configuration() -> Result<()> {
    let mut fixture = Fixture::new()?;
    let vmx_path = &fixture.vmx_path;

    create_vmdk(&fixture.disk_path, "10MB")?;

    // Create the initial virtual machine
    fixture.apply()?;

    // Update its settings and move the disk to SATA
    let display_name = "Updated Test VM";
    let vcpus = 6;
    let memory = 8192;
    let bus = "sata";

    fixture.ir["displayName"] = serde_json::json!(display_name);
    fixture.ir["vcpus"] = serde_json::json!(vcpus);
    fixture.ir["coresPerSocket"] = serde_json::Value::Null;
    fixture.ir["memory"] = serde_json::json!(memory);
    fixture.ir["secureBoot"] = serde_json::json!(false);
    fixture.ir["disks"]["primary"]["bus"] = serde_json::json!(bus);
    fixture.apply()?;

    assert::vmx_entry(vmx_path, "displayName", display_name)?;
    assert::vmx_entry(vmx_path, "numvcpus", &vcpus.to_string())?;
    assert::vmx_entry_absent(vmx_path, "cpuid.coresPerSocket")?;
    assert::vmx_entry(vmx_path, "memsize", &memory.to_string())?;
    assert::vmx_entry(vmx_path, "uefi.secureBoot.enabled", "FALSE")?;

    let snapshot = fixture.inspect()?;
    let disk_attachments = &snapshot.disk.disk_attachments;

    assert_eq!(disk_attachments.len(), 1);
    assert!(disk_attachments[0].label.as_ref().starts_with(bus));
    assert_eq!(
        disk_attachments[0].canonical_path,
        Some(fs::canonicalize(&fixture.disk_path)?)
    );

    // Reapplying automatic topology should change nothing
    fixture.assert_reapply_unchanged()?;

    Ok(())
}

#[test]
fn manages_display_configuration() -> Result<()> {
    let mut fixture = Fixture::new()?;
    let vmx_path = &fixture.vmx_path;
    let plist_path = &fixture.plist_path;

    // Apply configured display settings
    fixture.ir["display"] = serde_json::json!({
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
    fixture.apply()?;

    assert::vmx_entry_absent(vmx_path, "mks.enable3d")?;
    assert::vmx_entry(vmx_path, "svga.graphicsMemoryKB", "527360")?;
    assert::vmx_entry(vmx_path, "gui.perVMWindowAutofitMode", "stretch")?;
    assert::vmx_entry(vmx_path, "gui.perVMFullscreenAutofitMode", "center")?;
    assert::vmx_entry(vmx_path, "gui.fullScreenOnAllHostDisplays", "TRUE")?;
    assert::vmx_entry(vmx_path, "gui.fitGuestUsingNativeDisplayResolution", "TRUE")?;
    assert::plist_integer(plist_path, "scaledHighResolution", 2)?;

    // Apply another scaled high-resolution mode
    fixture.ir["display"]["nativeDisplayResolution"]["scaledHighResolution"] =
        serde_json::json!("full-screen");
    fixture.apply()?;

    assert::plist_integer(plist_path, "scaledHighResolution", 1)?;

    // Apply the default display settings
    fixture.ir["display"] = serde_json::json!({});
    fixture.apply()?;

    assert::vmx_entry(vmx_path, "mks.enable3d", "TRUE")?;
    assert::vmx_entry(vmx_path, "svga.graphicsMemoryKB", "262144")?;
    assert::vmx_entries_absent(
        vmx_path,
        &[
            "gui.perVMWindowAutofitMode",
            "gui.perVMFullscreenAutofitMode",
            "gui.fullScreenOnAllHostDisplays",
            "gui.fitGuestUsingNativeDisplayResolution",
        ],
    )?;
    assert::plist_entry_absent(plist_path, "scaledHighResolution")?;

    // Remove the display configuration
    fixture.ir["display"] = serde_json::Value::Null;
    fixture.apply()?;

    assert::vmx_entries_absent(
        vmx_path,
        &[
            "mks.enable3d",
            "svga.graphicsMemoryKB",
            "gui.perVMWindowAutofitMode",
            "gui.perVMFullscreenAutofitMode",
            "gui.fullScreenOnAllHostDisplays",
            "gui.fitGuestUsingNativeDisplayResolution",
        ],
    )?;
    assert::plist_entry_absent(plist_path, "scaledHighResolution")?;

    fixture.assert_reapply_unchanged()?;

    Ok(())
}

#[test]
fn manages_sound_card() -> Result<()> {
    let mut fixture = Fixture::new()?;
    let vmx_path = &fixture.vmx_path;

    // Apply the default sound card settings
    fixture.ir["sound"] = serde_json::json!({});
    fixture.apply()?;

    assert::vmx_entry(vmx_path, "sound.present", "TRUE")?;
    assert::vmx_entry(vmx_path, "sound.virtualDev", "hdaudio")?;
    assert::vmx_entry(vmx_path, "sound.autoDetect", "TRUE")?;
    assert::vmx_entry(vmx_path, "sound.fileName", "-1")?;
    assert::vmx_entry_absent(vmx_path, "sound.startConnected")?;
    assert::vmx_entry_absent(vmx_path, "sound.enableAEC")?;

    // Apply configured sound card settings
    fixture.ir["sound"]["startConnected"] = serde_json::json!(false);
    fixture.ir["sound"]["echoCancellation"] = serde_json::json!(true);
    fixture.apply()?;

    assert::vmx_entry(vmx_path, "sound.startConnected", "FALSE")?;
    assert::vmx_entry(vmx_path, "sound.enableAEC", "TRUE")?;

    // Simulate the PCI slot number VMware Fusion assigns to a sound card
    vmx::set_entry(vmx_path, "sound.pciSlotNumber", "515")?;

    // Remove the sound card
    fixture.ir["sound"] = serde_json::Value::Null;
    fixture.apply()?;

    assert::vmx_entries_absent(
        vmx_path,
        &[
            "sound.present",
            "sound.virtualDev",
            "sound.autoDetect",
            "sound.fileName",
            "sound.startConnected",
            "sound.enableAEC",
            "sound.pciSlotNumber",
        ],
    )?;

    fixture.assert_reapply_unchanged()?;

    Ok(())
}

#[test]
fn manages_usb_controller() -> Result<()> {
    let mut fixture = Fixture::new()?;
    let vmx_path = &fixture.vmx_path;

    // Apply the default USB controller configuration
    fixture.apply()?;

    assert::vmx_entry(vmx_path, "usb.present", "TRUE")?;
    assert::vmx_entry(vmx_path, "ehci.present", "TRUE")?;
    assert::vmx_entry(vmx_path, "usb_xhci.present", "TRUE")?;
    assert::vmx_entry_absent(vmx_path, "usb.generic.pluginAction")?;

    // Connect new USB devices to the virtual machine
    fixture.ir["usb"]["newDeviceAction"] = serde_json::json!("connect-to-vm");
    fixture.apply()?;

    assert::vmx_entry(vmx_path, "usb.generic.pluginAction", "guest")?;

    // Keep new USB devices connected to the host
    fixture.ir["usb"]["newDeviceAction"] = serde_json::json!("connect-to-host");
    fixture.apply()?;

    assert::vmx_entry(vmx_path, "usb.generic.pluginAction", "host")?;

    // Simulate the PCI slot number VMware Fusion assigns to the controller
    vmx::set_entry(vmx_path, "usb_xhci.pciSlotNumber", "515")?;

    // Remove the USB controller without removing VMware-owned entries
    fixture.ir["usb"] = serde_json::Value::Null;
    fixture.apply()?;

    assert::vmx_entry_absent(vmx_path, "usb.present")?;
    assert::vmx_entry_absent(vmx_path, "ehci.present")?;
    assert::vmx_entry_absent(vmx_path, "usb_xhci.present")?;
    assert::vmx_entry_absent(vmx_path, "usb.generic.pluginAction")?;
    assert::vmx_entry(vmx_path, "usb_xhci.pciSlotNumber", "515")?;

    // Restore the USB controller
    fixture.ir["usb"] = serde_json::json!({});
    fixture.apply()?;

    assert::vmx_entry(vmx_path, "usb.present", "TRUE")?;
    assert::vmx_entry(vmx_path, "ehci.present", "TRUE")?;
    assert::vmx_entry(vmx_path, "usb_xhci.present", "TRUE")?;
    assert::vmx_entry_absent(vmx_path, "usb.generic.pluginAction")?;
    assert::vmx_entry(vmx_path, "usb_xhci.pciSlotNumber", "515")?;

    fixture.assert_reapply_unchanged()?;

    Ok(())
}

#[test]
fn manages_optical_drive_lifecycle() -> Result<()> {
    let mut fixture = Fixture::new()?;
    let image_path = fixture.temp_dir.path().join("installer.iso");
    let updated_image_path = fixture.temp_dir.path().join("updated-installer.iso");

    fs::write(&image_path, [])?;
    fs::write(&updated_image_path, [])?;

    // Create the optical drive
    fixture.ir = serde_json::json!({
        "displayName": "Test VM",
        "path": fixture.bundle_path,
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
    fixture.apply()?;

    let snapshot = fixture.inspect()?;
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
    fixture.ir["opticalDrives"]["installer"]["source"]["path"] =
        serde_json::json!(updated_image_path);
    fixture.ir["opticalDrives"]["installer"]["startConnected"] = serde_json::json!(false);
    fixture.apply()?;

    let snapshot = fixture.inspect()?;
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
    fixture.ir["opticalDrives"] = serde_json::json!({});
    fixture.apply()?;

    let snapshot = fixture.inspect()?;

    assert!(snapshot.optical.optical_attachments.is_empty());
    assert!(snapshot.optical.state.optical_drives.is_empty());

    Ok(())
}

#[test]
fn renamed_bundle_uses_existing_vmx_and_plist_filenames() -> Result<()> {
    let mut fixture = Fixture::named("original")?;
    let renamed_bundle_path = fixture.temp_dir.path().join("renamed.vmwarevm");

    // Create the VM using its original bundle name
    fixture.ir["display"] = serde_json::json!({
        "nativeDisplayResolution": {
            "enable": true,
            "scaledHighResolution": "single-window"
        }
    });
    fixture.apply()?;

    // Rename only the bundle, leaving its internal filenames unchanged
    fs::rename(&fixture.bundle_path, &renamed_bundle_path)?;
    fixture.ir["path"] = serde_json::json!(renamed_bundle_path);
    fixture.ir["displayName"] = serde_json::json!("Renamed VM");
    fixture.ir["display"]["nativeDisplayResolution"]["scaledHighResolution"] =
        serde_json::json!("full-screen");
    fixture.apply()?;

    let vmx_path = renamed_bundle_path.join("original.vmx");
    let plist_path = renamed_bundle_path.join("original.plist");

    assert::vmx_entry(&vmx_path, "displayName", "Renamed VM")?;
    assert::plist_integer(&plist_path, "scaledHighResolution", 1)?;
    assert!(!renamed_bundle_path.join("renamed.vmx").try_exists()?);
    assert!(!renamed_bundle_path.join("renamed.plist").try_exists()?);

    Ok(())
}
