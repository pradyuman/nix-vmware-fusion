{
  dmg,
  lib,
  writeShellApplication,
}:

writeShellApplication {
  name = "vmware-fusion-install";

  text = ''
    source_dmg="${dmg}"
    target_app="/Applications/VMware Fusion.app"
    tmp_dir="$(/usr/bin/mktemp -d "''${TMPDIR:-/tmp}/nix-vmware-fusion.XXXXXX")"
    mount_dir="$tmp_dir/mount"

    # Temporary app copy verified before it is moved to /Applications.
    staged_app="$tmp_dir/VMware Fusion.app"

    # Backup path for an existing installation, if present.
    backup_app=""

    # Whether cleanup must eject the source DMG.
    mounted=0

    cleanup() {
      # Leave the temporary directory intact if the disk image cannot be ejected.
      if (( mounted )) && ! /usr/sbin/diskutil eject "$mount_dir" >/dev/null 2>&1; then
        echo "Could not detach the VMware Fusion disk image at $mount_dir" >&2
        return
      fi
      /bin/rm -rf "$tmp_dir"
    }
    trap cleanup EXIT

    restore_backup() {
      if [[ -n "$backup_app" ]]; then
        echo "Restoring the previous installation..." >&2
        /usr/bin/sudo /bin/mv "$backup_app" "$target_app"
      fi
    }

    # Replacing an application bundle while it is running is unsafe.
    if /usr/bin/pgrep -x "VMware Fusion" >/dev/null; then
      echo "VMware Fusion is running. Shut down its virtual machines and quit the app before installing." >&2
      exit 1
    fi

    /bin/mkdir "$mount_dir"

    echo "Mounting the VMware Fusion disk image..."
    /usr/sbin/diskutil image attach \
      --readOnly \
      --mountOptions nobrowse \
      --mountPoint "$mount_dir" \
      "$source_dmg" >/dev/null
    mounted=1

    echo "Copying VMware Fusion to a temporary location..."
    # ditto preserves the extended attributes used by VMware's signatures.
    /usr/bin/ditto "$mount_dir/VMware Fusion.app" "$staged_app"

    # Eject the DMG once the staged copy is complete.
    /usr/sbin/diskutil eject "$mount_dir" >/dev/null
    mounted=0
    /bin/rmdir "$mount_dir"

    echo "Verifying VMware Fusion's Apple signature..."
    /usr/bin/codesign --verify --deep --strict --verbose=2 "$staged_app"
    /usr/sbin/spctl --assess --type execute --verbose=2 "$staged_app"

    # Ask for administrator access only after the staged app is verified.
    /usr/bin/sudo -v

    # Preserve an existing installation so it can be restored on failure.
    if [[ -e "$target_app" || -L "$target_app" ]]; then
      backup_app="$target_app.nix-backup-$(/bin/date +%Y%m%d%H%M%S)-$$"
      echo "Moving the existing installation to $backup_app..."
      /usr/bin/sudo /bin/mv "$target_app" "$backup_app"
    fi

    echo "Installing VMware Fusion in /Applications..."
    if ! /usr/bin/sudo /bin/mv "$staged_app" "$target_app"; then
      restore_backup
      exit 1
    fi

    echo "Initializing VMware Fusion..."
    # VMware's signed, bundled one-time setup mechanism.
    if ! /usr/bin/sudo "$target_app/Contents/Library/Initialize VMware Fusion.tool" set; then
      echo "Initialization failed; removing the new installation..." >&2
      /usr/bin/sudo /bin/rm -rf "$target_app"
      restore_backup
      exit 1
    fi

    if [[ -n "$backup_app" ]]; then
      echo "Removing the backup..."
      /usr/bin/sudo /bin/rm -rf "$backup_app"
    fi

    echo "VMware Fusion is installed and initialized at $target_app"
  '';

  meta = {
    description = "Install and initialize VMware Fusion";
    license = lib.licenses.isc;
    platforms = lib.platforms.darwin;
    mainProgram = "vmware-fusion-install";
  };
}
