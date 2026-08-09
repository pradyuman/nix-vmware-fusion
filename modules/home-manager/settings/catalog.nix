# τ SettingValue = Bool | Int | String
#
# τ Write = {
#   preferences? :: { String = SettingValue; };
#   removePreferences? :: [ String ];
#   darwinDefaults? :: { String = SettingValue; };
# }
#
# τ BoolSetting = {
#   type :: "bool";
#   description :: String;
#   write :: Bool -> Write;
# }
#
# τ EnumSetting = {
#   type :: "enum";
#   values :: [ String ];
#   description :: String;
#   write :: String -> Write;
# }
#
# τ Catalog = { String = BoolSetting | EnumSetting; }

let
  writePreference = name: encode: value: {
    preferences = {
      ${name} = encode value;
    };
  };

  writeDarwinDefault = name: encode: value: {
    darwinDefaults = {
      ${name} = encode value;
    };
  };
in
{
  appearance = {
    type = "enum";
    values = [
      "auto"
      "light"
      "dark"
    ];
    description = "VMware Fusion's appearance.";
    write = writeDarwinDefault "fusionAppearance" (
      value:
      builtins.getAttr value {
        auto = 0;
        light = 1;
        dark = 2;
      }
    );
  };

  closeAction = {
    type = "enum";
    values = [
      "suspend"
      "power-off"
    ];
    description = "Action to take when closing a virtual machine window.";
    write = writePreference "pref.vmplayer.exit.vmAction" (
      value:
      builtins.getAttr value {
        suspend = "suspend";
        power-off = "poweroff";
      }
    );
  };

  confirmBeforeClosing = {
    type = "bool";
    description = "Whether to confirm before closing a virtual machine or quitting VMware Fusion.";
    write = writePreference "pref.vmplayer.confirmOnExit" (value: value);
  };

  dataCollectionEnabled = {
    type = "bool";
    description = "Whether to participate in VMware's Customer Experience Improvement Program.";
    write = writePreference "pref.dataCollectionEnabled" (value: value);
  };

  fullScreenMode = {
    type = "enum";
    values = [
      "center"
      "stretch"
      "fit"
    ];
    description = "How to size a virtual machine in full screen.";
    write = writePreference "pref.autoFitFullScreen" (
      value:
      builtins.getAttr value {
        center = "none";
        stretch = "stretchGuestToHost";
        fit = "fitGuestToHost";
      }
    );
  };

  gamingMouseMode = {
    type = "enum";
    values = [
      "auto"
      "never"
      "always"
    ];
    description = "When to optimize the mouse for games.";
    write =
      value:
      if value == "auto" then
        {
          removePreferences = [ "pref.gamingMouseMode" ];
        }
      else
        writePreference "pref.gamingMouseMode" (
          mode:
          builtins.getAttr mode {
            never = "absoluteMouse";
            always = "relativeMouse";
          }
        ) value;
  };

  perVirtualMachineShortcuts = {
    type = "bool";
    description = "Whether to enable per-virtual machine keyboard shortcuts.";
    write = writePreference "pref.keyboardAndMouse.vmHotKey.enabled" (value: value);
  };

  singleWindowMode = {
    type = "enum";
    values = [
      "stretch"
      "resize"
    ];
    description = "How to size a virtual machine in a single window.";
    write = value: {
      preferences = {
        "pref.autoFit" = value == "resize";
        "pref.autoFitGuestToWindow" = value == "resize";
      };
    };
  };
}
