#![cfg(feature = "vmware-tests")]

use std::fs;

use anyhow::Result;

use support::CONFIG;

mod support;

#[test]
fn cli_applies_virtual_machine_configuration() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let bundle_path = temp_dir.path().join("test.vmwarevm");
    let ir_path = temp_dir.path().join("virtual-machine-ir.json");
    let vmx_path = bundle_path.join("test.vmx");

    let ir = serde_json::json!({
        "displayName": "Test VM",
        "path": bundle_path,
        "guestOS": "arm-other6xlinux-64",
        "vcpus": 2,
        "memory": 4096,
        "secureBoot": false,
        "disks": {}
    });
    fs::write(&ir_path, serde_json::to_vec(&ir)?)?;

    duct::cmd!(&CONFIG.cli, "vm", "apply", &ir_path).run()?;

    assert!(vmx_path.is_file());

    let display_name =
        duct::cmd!(&CONFIG.dict_tool, "-q", "query", &vmx_path, "displayName").read()?;

    assert_eq!(display_name, r#"displayName = "Test VM""#);

    Ok(())
}
