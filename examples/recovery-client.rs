//! Test-only exec gate for repeated locker failure/recovery; no authentication.
use anyhow::{Context, Result};
use clap::Parser;
use std::{ffi::OsString, os::unix::process::CommandExt, path::PathBuf};
#[derive(Parser)]
struct Options {
    #[arg(long)]
    allow_file: PathBuf,
    #[arg(long)]
    program: PathBuf,
    #[arg(last = true)]
    arguments: Vec<OsString>,
}
fn main() -> Result<()> {
    let options = Options::parse();
    if !options.allow_file.try_exists()? {
        std::process::exit(1);
    }
    // Do not adopt/close WAYLAND_SOCKET. The configured fixture/real locker
    // inherits exactly the parent's private connection and the same PID.
    let error = std::process::Command::new(options.program)
        .args(options.arguments)
        .exec();
    Err(error).context("exec controlled recovery-test client")
}
