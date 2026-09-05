#!/usr/bin/env bash
set -euo pipefail

die() {
  echo "$*" >&2
  exit 1
}

main() {
  local target_app="/Applications/VMware Fusion.app"
  local shared_support="/Library/Application Support/VMware"
  local fusion_support="$shared_support/VMware Fusion"
  local services_script="$fusion_support/Services/Contents/Library/services/services.sh"

  # Resolve the selected user's home directory.
  local current_user
  current_user="$(/usr/bin/id -un)"

  local skip_confirmation="${1:?expected confirmation flag}"
  local purge_user="${2:-}"

  if [[ -z "$purge_user" ]]; then
    if (( EUID == 0 )); then
      die "When running as root, select a user with --user."
    fi
    purge_user="$current_user"
  elif (( EUID != 0 )) && [[ "$purge_user" != "$current_user" ]]; then
    die "Only root can select another user."
  fi

  if ! /usr/bin/id "$purge_user" >/dev/null 2>&1; then
    die "Selected user $purge_user does not exist."
  fi

  local user_home
  user_home="$(
    /usr/bin/dscl . -read "/Users/$purge_user" NFSHomeDirectory \
      | /usr/bin/sed 's/^NFSHomeDirectory: //'
  )"

  if [[ -z "$user_home" || "$user_home" != /* || "$user_home" == "/" ]]; then
    die "Could not resolve a safe home directory for $purge_user."
  fi

  local user_library="$user_home/Library"

  # Refuse to purge while VMware Fusion or a virtual machine is running.
  if /usr/bin/pgrep -x "VMware Fusion" >/dev/null \
    || /usr/bin/pgrep -f "/VMware Fusion.app/Contents/Library/vmware-vmx" >/dev/null; then
    die "VMware Fusion or one of its virtual machines is running. Shut down its virtual machines and quit the app before purging."
  fi

  # Preserve shared support files when another VMware product may use them.
  local has_other_vmware_support=0
  local support_path
  shopt -s dotglob nullglob
  for support_path in "$shared_support"/*; do
    case "${support_path##*/}" in
      "VMware Fusion" | usbarb.rules) ;;
      *)
        has_other_vmware_support=1
        break
        ;;
    esac
  done

  echo "This will remove VMware Fusion, as well as its settings, logs, and privileged helpers."
  if (( has_other_vmware_support )); then
    echo "Other VMware support files were found and will be preserved."
  else
    echo "Shared VMware support files will also be removed."
  fi
  echo "Existing virtual machine bundles will not be touched."

  if [[ "$skip_confirmation" != "true" ]]; then
    echo
    if ! gum confirm --default=false "Continue?"; then
      echo "Purge cancelled"
      exit 0
    fi
  fi

  # Stop VMware services and remove privileged helpers.
  /usr/bin/sudo -v

  if [[ -x "$services_script" ]]; then
    echo "Stopping VMware Fusion services..."
    /usr/bin/sudo "$services_script" --stop
  fi

  local helper
  local helper_plist
  local helper_tool
  for helper in \
    com.vmware.DiskHelper \
    com.vmware.IDHelper \
    com.vmware.MountHelper \
    com.vmware.fusion.InstallHelper; do
    helper_plist="/Library/LaunchDaemons/$helper.plist"
    helper_tool="/Library/PrivilegedHelperTools/$helper"

    if [[ -e "$helper_plist" ]]; then
      /usr/bin/sudo /bin/launchctl bootout system "$helper_plist" >/dev/null 2>&1 || true
    fi

    /usr/bin/sudo /bin/rm -f "$helper_plist" "$helper_tool"
  done

  echo "Removing VMware Fusion system files..."
  /usr/bin/sudo /bin/rm -rf \
    "$target_app" \
    "$fusion_support" \
    "/Library/Application Support/VMware Fusion" \
    "/Library/Preferences/VMware Fusion" \
    "/Library/Logs/VMware Fusion Services.log" \
    "/etc/paths.d/com.vmware.fusion.public"

  if (( ! has_other_vmware_support )); then
    echo "Removing shared VMware support files..."
    /usr/bin/sudo /bin/rm -f "$shared_support/usbarb.rules"
    /usr/bin/sudo /bin/rmdir "$shared_support" 2>/dev/null || true
  fi

  # Remove preferences and other user-specific files.
  # `defaults delete` also clears preferences cached by macOS. Their files
  # are removed explicitly below in case any remain on disk.
  local domain
  for domain in \
    com.vmware.fusion \
    com.vmware.fusionApplicationsMenu \
    com.vmware.fusionDaemon \
    com.vmware.fusionStartMenu; do
    /usr/bin/sudo -u "$purge_user" -H /usr/bin/defaults delete "$domain" >/dev/null 2>&1 || true
  done

  local -a user_paths=(
    "$user_library/Application Support/VMware Fusion"
    "$user_library/Application Support/VMware Fusion Applications Menu"
    "$user_library/Application Support/com.apple.sharedfilelist/com.apple.LSSharedFileList.ApplicationRecentDocuments/com.vmware.fusion.sfl"*
    "$user_library/Caches/com.vmware.fusion"
    "$user_library/Logs/VMware Fusion"
    "$user_library/Logs/VMware Fusion Applications Menu"
    "$user_library/Preferences/VMware Fusion"
    "$user_library/Preferences/com.vmware.fusion.LSSharedFileList.plist"
    "$user_library/Preferences/com.vmware.fusion.LSSharedFileList.plist.lockfile"
    "$user_library/Preferences/com.vmware.fusion.plist"
    "$user_library/Preferences/com.vmware.fusion.plist.lockfile"
    "$user_library/Preferences/com.vmware.fusionApplicationsMenu.helper.plist"
    "$user_library/Preferences/com.vmware.fusionApplicationsMenu.plist"
    "$user_library/Preferences/com.vmware.fusionDaemon.plist"
    "$user_library/Preferences/com.vmware.fusionDaemon.plist.lockfile"
    "$user_library/Preferences/com.vmware.fusionStartMenu.plist"
    "$user_library/Preferences/com.vmware.fusionStartMenu.plist.lockfile"
    "$user_library/Saved Application State/com.vmware.fusion.savedState"
    "$user_library/WebKit/com.vmware.fusion"
  )

  echo "Removing the selected user's VMware Fusion files ($purge_user)..."
  /usr/bin/sudo /bin/rm -rf -- "${user_paths[@]}"

  echo "VMware Fusion has been purged"
}

main "$@"
