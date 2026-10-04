macro_rules! config {
    ($($field:ident),+ $(,)?) => {
        #[derive(serde::Deserialize)]
        struct Config {
            $(
                $field: std::path::PathBuf,
            )+
        }

        static CONFIG: std::sync::LazyLock<Config> = std::sync::LazyLock::new(|| {
            ::config::Config::builder()
                .add_source(::config::Environment::with_prefix("NIX_VMWARE_FUSION"))
                .set_override("cli", env!("CARGO_BIN_EXE_nix-vmware-fusion"))
                .and_then(|builder| builder.build())
                .and_then(|config| config.try_deserialize())
                .expect("could not load test configuration")
        });
    };
}

pub(super) use config;
