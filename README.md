# nix-vmware-fusion

Manage VMware Fusion on macOS with Nix.

The flake is currently pinned to VMware Fusion `26H1u1` (build `25689522`) for
Macs with Apple silicon.

## Install

### 1. Download VMware Fusion

Sign in to the [VMware Fusion 26H1u1 downloads page][fusion-downloads] and
download this file:

```text
VMware-Fusion-26H1u1-25689522_universal.dmg
```

If prompted, review and accept Broadcom's Terms and Conditions and complete the
Trade Compliance form. Broadcom's [download instructions][broadcom-download-instructions]
explain these steps.

### 2. Add the DMG to the Nix store

```sh
# Replace this path if the DMG is elsewhere.
dmg="$HOME/Downloads/VMware-Fusion-26H1u1-25689522_universal.dmg"
nix store add \
  --mode flat \
  --hash-algo sha256 \
  --name VMware-Fusion-26H1u1-25689522_universal.dmg \
  "$dmg"
```

> [!NOTE]
> You will only need to import the DMG once per pinned VMware Fusion build.

### 3. Install VMware Fusion

#### With nix-darwin

Add the module to your nix-darwin configuration:

```nix
{
  inputs.nix-vmware-fusion = {
    url = "github:pradyuman/nix-vmware-fusion";
    inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { nix-darwin, nix-vmware-fusion, ... }: {
    darwinConfigurations.my-mac = nix-darwin.lib.darwinSystem {
      modules = [
        nix-vmware-fusion.darwinModules.default
        {
          programs.vmware-fusion.enable = true;
        }
      ];
    };
  };
}
```

On activation, the module installs the pinned build and adds the following to
the system profile:

- `nix-vmware-fusion`, with `install`, `uninstall`, and `purge` subcommands
- VMware Fusion's bundled command-line tools, including `vmrun`, `vmcli`,
  `vmrest`, `vmnet-cli`, and `ovftool`

To manage VMware Fusion's networking config, set:

```nix
programs.vmware-fusion.networking.text = ''
  VERSION=1,0
  answer VNET_1_DHCP yes
  answer VNET_1_HOSTONLY_NETMASK 255.255.255.0
  answer VNET_1_HOSTONLY_SUBNET 192.168.200.0
  answer VNET_1_VIRTUAL_ADAPTER yes
  ...
'';
```

The module sets the new configuration during activation, but you will need to
restart VMware's networking services for the changes to take effect.

#### Directly

If you do not use nix-darwin, you can run the installer directly:

```sh
NIXPKGS_ALLOW_UNFREE=1 nix run --impure github:pradyuman/nix-vmware-fusion -- install
```

The installer uses `sudo` to install the app in `/Applications` and initialize
VMware's privileged helpers.

## Manage user preferences

To manage user preferences, use the Home Manager module:

```nix
{
  imports = [ nix-vmware-fusion.homeModules.default ];

  programs.vmware-fusion = {
    enable = true;
    settings = {
      appearance = "dark";
      confirmBeforeClosing = true;
      dataCollectionEnabled = false;
      fullScreenMode = "fit";
      mapISONumpadEnterToAltGrEnabled = false;
    };
  };
}
```

The module supports these settings:

| Setting                      | Values                           | Description                                                    |
| ---------------------------- | -------------------------------- | -------------------------------------------------------------- |
| `appearance`                 | `"auto"`, `"light"`, `"dark"`    | VMware Fusion's appearance.                                    |
| `closeAction`                | `"suspend"`, `"power-off"`       | Action to take when closing a virtual machine window.          |
| `confirmBeforeClosing`       | `true`, `false`                  | Whether to confirm before closing a virtual machine or Fusion. |
| `dataCollectionEnabled`      | `true`, `false`                  | Whether to participate in VMware's data collection program.    |
| `fullScreenMode`             | `"center"`, `"stretch"`, `"fit"` | How to size a virtual machine in full screen.                  |
| `gamingMouseMode`            | `"auto"`, `"never"`, `"always"`  | When to optimize the mouse for games.                          |
| `perVirtualMachineShortcuts` | `true`, `false`                  | Whether to enable per-virtual machine keyboard shortcuts.      |
| `singleWindowMode`           | `"stretch"`, `"resize"`          | How to size a virtual machine in a single window.              |

To manage a setting that is not listed above, add its VMware preference key to
`settings` without the `pref.` prefix. Settings you do not specify are left
unchanged.

Quit VMware Fusion before activating your Home Manager configuration, then
reopen it afterward so the new settings take effect.

## Manage virtual machines

To create a VM or configure an existing one, use the Home Manager module:

```nix
programs.vmware-fusion = {
  enable = true;
  virtualMachines.asuna = {
    guestOS = "arm-other6xlinux-64";
    vcpus = 2;
    memory = 8192;
    secureBoot = false;
    disks.primary = {
      size = 64;
      bus = "nvme";
    };
  };
};
```

By default, the module creates or updates
`~/Virtual Machines.localized/<name>.vmwarevm`,
where `<name>` is the attribute name (`asuna` above). Use `path` to specify a
different location.

The module only manages virtual machines listed in your configuration and
leaves any others on your system untouched. If you remove a virtual machine
from your configuration, the module stops managing it (but doesn't delete it).

> [!IMPORTANT]
> `nix-vmware-fusion` will return an error if it tries to change the
> configuration of a running virtual machine. Make sure to shut down any
> virtual machines you're changing before activating a new configuration.

### Virtual disks

Declare every virtual disk that should be attached to the VM:

```nix
programs.vmware-fusion.virtualMachines.asuna = {
  # ...
  disks = {
    primary = {
      size = 64;
      bus = "nvme";
      preallocate = false;
      split = true;
    };
  };
};
```

Each disk supports the following settings:

| Setting       | Type                 | Default                | Description                                                                        |
| ------------- | -------------------- | ---------------------- | ---------------------------------------------------------------------------------- |
| `path`        | String               | `<bundle>/<name>.vmdk` | Disk location, where `<name>` is the attribute name (`primary` above)              |
| `size`        | Positive integer     | —                      | Disk capacity in GiB (required)                                                    |
| `bus`         | `"nvme"` or `"sata"` | `"nvme"`               | Bus used to attach the disk                                                        |
| `preallocate` | Boolean              | `false`                | Reserve the disk's full capacity on the host instead of growing as data is written |
| `split`       | Boolean              | `false`                | Store the disk across multiple files instead of a single file                      |

When changing disk settings, keep in mind:

- You can [expand a disk, but not shrink it][fusion-resize-virtual-disk].
- Expanding a disk does not [resize its partitions or filesystems inside the
  guest][vmware-resize-partition].
- Changing `preallocate` or `split` will [convert the virtual disk to the target
  format][vmware-convert-virtual-disk].

Removing a disk declaration will detach it from the VM without deleting its
files.

> [!NOTE]
> When adding an existing VM to your configuration, make sure to declare any
> existing disks unless you want them detached.

## Remove VMware Fusion

### Uninstall

By default, disabling the nix-darwin module does not remove VMware Fusion. To
uninstall during activation when the module is disabled, set:

```nix
programs.vmware-fusion = {
  enable = false;
  onActivation.cleanup = "uninstall";
};
```

You can also run the uninstaller directly:

```sh
NIXPKGS_ALLOW_UNFREE=1 nix run --impure github:pradyuman/nix-vmware-fusion -- uninstall
```

The uninstaller removes `/Applications/VMware Fusion.app` but leaves your
virtual machines, preferences, and privileged helpers in place.

### Purge

Unlike uninstall (which only removes the app), purging also removes VMware
Fusion's system support files, services, privileged helpers, and the selected
user's preferences and logs. It does not touch any existing virtual machine
bundles.

To purge during activation when the nix-darwin module is disabled, set:

```nix
system.primaryUser = "my-user";

programs.vmware-fusion = {
  enable = false;
  onActivation.cleanup = "purge";
};
```

You can also run the command directly:

```sh
NIXPKGS_ALLOW_UNFREE=1 nix run --impure github:pradyuman/nix-vmware-fusion -- purge
```

Purging also removes `usbarb.rules` if the VMware application support directory
contains no entries from other VMware programs.

## Why not a normal package?

Some files in VMware's app bundle store code-signature data in macOS extended
attributes. Because the [Nix archive format][nix-archive-format] does not
preserve those attributes, storing the extracted app directly in the Nix store
would discard the signature data (which means strict code-signature verification
would fail even though the files themselves were unchanged).

So instead, the flake stores the original DMG as an opaque file. At runtime, it
mounts the DMG and copies the app with `ditto` (which preserves the extended
attributes), and then verifies the copy before installing and initializing it.

## Licensing

The code in this repository is licensed under the [ISC License](LICENSE). That
license does not apply to VMware Fusion.

Broadcom currently makes the supported VMware Fusion release available at no
charge for personal, educational, and commercial use. Nevertheless, VMware
Fusion remains proprietary software. Downloading, installing, and using it is
subject to Broadcom's applicable [licensing terms][broadcom-licensing], product-specific
terms, and any conditions shown during download.

[broadcom-download-instructions]: https://knowledge.broadcom.com/external/article/368667/download-and-license-vmware-desktop-hype.html
[broadcom-licensing]: https://www.broadcom.com/company/legal/licensing
[fusion-downloads]: https://support.broadcom.com/group/ecx/productfiles?subFamily=VMware%20Fusion&displayGroup=VMware%20Fusion%2026H1&release=26H1u1&os=&servicePk=546858&language=EN&freeDownloads=true
[fusion-resize-virtual-disk]: https://techdocs2-prod.adobecqms.net/content/dam/broadcom/techdocs/us/en/pdf/vmware/desktop-hypervisors/fusion/vmware-fusion-pro-13.pdf
[nix-archive-format]: https://nix.dev/manual/nix/latest/protocols/nix-archive
[vmware-convert-virtual-disk]: https://knowledge.broadcom.com/external/article/302738
[vmware-resize-partition]: https://knowledge.broadcom.com/external/article?legacyId=1004071
