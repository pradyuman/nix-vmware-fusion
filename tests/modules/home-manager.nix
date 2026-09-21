{
  nixpkgs,
  homeManager,
}:

let
  lib = import "${nixpkgs}/lib";
  homeManagerLib = import "${homeManager}/lib" { inherit lib; };
  pkgs = import nixpkgs {
    localSystem = "aarch64-darwin";

    # Evaluate the module without requiring the proprietary DMG.
    overlays = [
      (_final: prev: {
        requireFile =
          _:
          prev.runCommand "mock-vmware-fusion-dmg" { } ''
            touch "$out"
          '';
      })
    ];
  };

  mkHome =
    vmwareFusionConfig:
    homeManagerLib.homeManagerConfiguration {
      inherit pkgs;
      modules = [
        ../../modules/home-manager
        {
          home = {
            username = "test";
            homeDirectory = "/Users/test";
            stateVersion = "26.05";
          };
          programs.vmware-fusion = vmwareFusionConfig;
        }
      ];
    };

  defaultHome = mkHome { };
  configuredHome = mkHome {
    enable = true;
    settings = {
      appearance = "dark";
      boolean = false;
      closeAction = "power-off";
      confirmBeforeClosing = true;
      dataCollectionEnabled = false;
      fullScreenMode = "stretch";
      gamingMouseMode = "never";
      integer = 42;
      perVirtualMachineShortcuts = true;
      singleWindowMode = "stretch";
      string = "hello world";
    };
  };

  automaticGamingHome = mkHome {
    enable = true;
    settings.gamingMouseMode = "auto";
  };

  overriddenHome = mkHome {
    enable = true;
    settings = {
      closeAction = "power-off";
      "vmplayer.exit.vmAction" = "suspend";
    };
  };
  virtualMachineHome = mkHome {
    enable = true;
    virtualMachines.asuna = {
      guestOS = "arm-other6xlinux-64";
      vcpus = 4;
      memory = 8192;
      secureBoot = true;
      disks.primary.size = 64;
    };
  };

  getActivation = home: home.config.home.activation.vmwareFusionPreferences.data;

  configuredActivation = getActivation configuredHome;
  automaticGamingActivation = getActivation automaticGamingHome;
  overriddenActivation = getActivation overriddenHome;
  asuna = virtualMachineHome.config.programs.vmware-fusion.virtualMachines.asuna;
  virtualMachineActivation =
    virtualMachineHome.config.home.activation.vmwareFusionVirtualMachines.data;
in
{
  testDisabledByDefault = {
    expr = defaultHome.config.programs.vmware-fusion.enable;
    expected = false;
  };

  testFreeformSettingsAreSet = {
    expr = builtins.all (command: lib.hasInfix command configuredActivation) [
      "pref.boolean=FALSE"
      "pref.integer=42"
      "pref.string=hello world"
    ];
    expected = true;
  };

  testSettingsAreEncoded = {
    expr = builtins.all (preference: lib.hasInfix preference configuredActivation) [
      "pref.vmplayer.exit.vmAction=poweroff"
      "pref.vmplayer.confirmOnExit=TRUE"
      "pref.dataCollectionEnabled=FALSE"
      "pref.autoFitFullScreen=stretchGuestToHost"
      "pref.gamingMouseMode=absoluteMouse"
      "pref.keyboardAndMouse.vmHotKey.enabled=TRUE"
      "pref.autoFit=FALSE"
      "pref.autoFitGuestToWindow=FALSE"
    ];
    expected = true;
  };

  testAppearanceUsesDarwinDefaults = {
    expr = configuredHome.config.targets.darwin.defaults."com.vmware.fusion".fusionAppearance;
    expected = 2;
  };

  testAutomaticGamingRemovesPreference = {
    expr =
      lib.hasInfix "dictTool remove" automaticGamingActivation
      && lib.hasInfix "pref.gamingMouseMode" automaticGamingActivation;
    expected = true;
  };

  testSettingsTakePrecedence = {
    expr =
      lib.hasInfix "pref.vmplayer.exit.vmAction=poweroff" overriddenActivation
      && !(lib.hasInfix "pref.vmplayer.exit.vmAction=suspend" overriddenActivation);
    expected = true;
  };

  testInvalidSetting = {
    expr =
      (mkHome {
        enable = true;
        settings.appearance = "sepia";
      }).config.programs.vmware-fusion.settings.appearance;
    expectedError.type = "ThrownError";
  };

  testVirtualMachineDefaults = {
    expr = asuna;
    expected = {
      displayName = "asuna";
      path = "/Users/test/Virtual Machines.localized/asuna.vmwarevm";
      guestOS = "arm-other6xlinux-64";
      vcpus = 4;
      memory = 8192;
      secureBoot = true;
      disks.primary = {
        path = "/Users/test/Virtual Machines.localized/asuna.vmwarevm/primary.vmdk";
        size = 64;
        bus = "nvme";
        preallocate = false;
        split = false;
      };
    };
  };

  testVirtualMachineActivation = {
    expr =
      lib.hasInfix "/bin/nix-vmware-fusion vm apply" virtualMachineActivation
      && lib.hasInfix "nix-vmware-fusion-asuna-ir.json" virtualMachineActivation;
    expected = true;
  };
}
