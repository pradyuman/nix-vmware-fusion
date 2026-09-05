#!/usr/bin/env bash
set -euo pipefail

target_app="/Applications/VMware Fusion.app"

# Removing an application bundle while it is running is unsafe.
if /usr/bin/pgrep -x "VMware Fusion" >/dev/null; then
  echo "VMware Fusion is running. Shut down its virtual machines and quit the app before uninstalling." >&2
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
