{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.programs.vmware-fusion;
  localPkgs = import ../../../pkgs { inherit pkgs; };

  catalog = import ./catalog.nix;
  catalogNames = builtins.attrNames catalog;
  configuredCatalogSettings = lib.filterAttrs (_: value: value != null) (
    builtins.intersectAttrs catalog cfg.settings
  );
  freeformSettings = builtins.removeAttrs cfg.settings catalogNames;
  freeformPreferences = lib.mapAttrs' (
    name: value: lib.nameValuePair "pref.${name}" value
  ) freeformSettings;
  writes = lib.mapAttrsToList (name: value: catalog.${name}.write value) configuredCatalogSettings;

  mergeChanges = field: lib.mergeAttrsList (map (write: write.${field} or { }) writes);

  catalogPreferences = mergeChanges "preferences";
  removePreferences = lib.unique (lib.concatMap (write: write.removePreferences or [ ]) writes);
  preferences = (builtins.removeAttrs freeformPreferences removePreferences) // catalogPreferences;

  darwinDefaults = mergeChanges "darwinDefaults";

  dictTool = lib.getExe' localPkgs.commandLineTools "dictTool";
  preferencesFile = "${config.home.homeDirectory}/Library/Preferences/VMware Fusion/preferences";

  renderPreference =
    value: if builtins.isBool value then if value then "TRUE" else "FALSE" else toString value;
in
{
  imports = [ ./options.nix ];

  config = lib.mkIf cfg.enable {
    home.activation.vmwareFusionPreferences =
      lib.mkIf (preferences != { } || removePreferences != [ ])
        (
          lib.hm.dag.entryAfter [ "writeBoundary" ] ''
            run /bin/mkdir -p ${lib.escapeShellArg (builtins.dirOf preferencesFile)}

            if [[ ! -e ${lib.escapeShellArg preferencesFile} ]]; then
              run /usr/bin/touch ${lib.escapeShellArg preferencesFile}
            fi

            ${lib.optionalString (removePreferences != [ ]) ''
              for name in ${lib.escapeShellArgs removePreferences}; do
                if ${dictTool} query ${lib.escapeShellArg preferencesFile} "$name" >/dev/null 2>&1; then
                  run --quiet ${dictTool} remove ${lib.escapeShellArg preferencesFile} "$name"
                fi
              done
            ''}

            ${lib.concatMapAttrsStringSep "\n" (
              name: value:
              "run --quiet ${dictTool} set ${lib.escapeShellArg preferencesFile} ${lib.escapeShellArg "${name}=${renderPreference value}"}"
            ) preferences}
          ''
        );

    targets.darwin.defaults."com.vmware.fusion" = lib.mkIf (darwinDefaults != { }) darwinDefaults;
  };
}
