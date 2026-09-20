use anyhow::Result;
use std::ffi::OsString;
use std::process::ExitCode;

pub(crate) fn run(name: &str, contents: &str, extra_args: &[OsString]) -> Result<ExitCode> {
    let mut args = vec![OsString::from("-c"), contents.into(), name.into()];
    args.extend_from_slice(extra_args);

    let code = duct::cmd("bash", args).unchecked().run()?.status.code();
    Ok(code.map_or(ExitCode::FAILURE, |code| ExitCode::from(code as u8)))
}
