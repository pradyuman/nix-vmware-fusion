{ lib, ... }:

let
  catalog = import ./catalog.nix;
  topLevelCatalog = builtins.removeAttrs catalog [ "display" ];

  settingValueType = lib.types.oneOf [
    lib.types.bool
    lib.types.int
    lib.types.str
  ];

  settingOptionType =
    setting: if setting.type == "bool" then lib.types.bool else lib.types.enum setting.values;

  settingOptions =
    catalog':
    lib.mapAttrs (
      _: setting:
      lib.mkOption {
        type = lib.types.nullOr (settingOptionType setting);
        default = null;
        inherit (setting) description;
      }
    ) catalog';
in
{
  options.programs.vmware-fusion.settings = lib.mkOption {
    type = lib.types.submodule {
      freeformType = lib.types.attrsOf settingValueType;
      options = (settingOptions topLevelCatalog) // {
        display = lib.mkOption {
          type = lib.types.submodule {
            options = settingOptions catalog.display;
          };
          default = { };
          description = "Application-level display preferences.";
        };
      };
    };
    default = { };
    description = ''
      VMware Fusion settings. Attributes not defined by this module are
      written as VMware preference keys with the `pref.` prefix. Settings not
      specified here are left unchanged.
    '';
  };
}
