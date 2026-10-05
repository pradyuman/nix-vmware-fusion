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
# τ Catalog = {
#   display :: { String = BoolSetting | EnumSetting; };
#   String = BoolSetting | EnumSetting;
# }

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

  dataCollectionEnabled = {
    type = "bool";
    description = "Whether to participate in VMware's Customer Experience Improvement Program.";
    write = writePreference "pref.dataCollectionEnabled" (value: value);
  };

  perVmKeyboardShortcutsEnabled = {
    type = "bool";
    description = "Whether to enable per-virtual machine keyboard shortcuts.";
    write = writePreference "pref.keyboardAndMouse.vmHotKey.enabled" (value: value);
  };

  display = {
    singleWindowFit = {
      type = "enum";
      values = [
        "stretch"
        "resize"
      ];
      description = "How to size the virtual machine display in single-window mode.";
      write = value: {
        preferences = {
          "pref.autoFit" = value == "resize";
          "pref.autoFitGuestToWindow" = value == "resize";
        };
      };
    };

    fullScreenFit = {
      type = "enum";
      values = [
        "center"
        "stretch"
        "resize"
      ];
      description = "How to size the virtual machine display in full-screen mode.";
      write = writePreference "pref.autoFitFullScreen" (
        value:
        builtins.getAttr value {
          center = "none";
          stretch = "stretchGuestToHost";
          resize = "fitGuestToHost";
        }
      );
    };
  };

}
