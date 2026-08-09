{
  nixpkgs,
  homeManager,
}:

let
  lib = import "${nixpkgs}/lib";
  homeManagerLib = import "${homeManager}/lib" { inherit lib; };
  pkgs = import nixpkgs { localSystem = "aarch64-darwin"; };

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

  getActivation = home: home.config.home.activation.vmwareFusionPreferences.data;

  configuredActivation = getActivation configuredHome;
  automaticGamingActivation = getActivation automaticGamingHome;
  overriddenActivation = getActivation overriddenHome;
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
}
