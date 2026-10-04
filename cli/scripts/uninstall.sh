#!/usr/bin/env bash
set -euo pipefail

target_app="/Applications/VMware Fusion.app"

# Removing an application bundle while Fusion or one of its virtual machines is running is unsafe.
if /usr/bin/pgrep -x "VMware Fusion" >/dev/null \
  || /usr/bin/pgrep -x "vmware-vmx" >/dev/null; then
  echo "VMware Fusion or one of its virtual machines is running. Shut down its virtual machines and quit the app before uninstalling." >&2
  exit 1
fi

if [[ ! -e "$target_app" && ! -L "$target_app" ]]; then
  echo "VMware Fusion is not installed at $target_app"
  exit 0
fi

/usr/bin/sudo -v

echo "Removing VMware Fusion from /Applications..."
/usr/bin/sudo /bin/rm -rf "$target_app"

echo "VMware Fusion has been removed from /Applications"
echo "Virtual machines, preferences, and privileged helpers were preserved"
