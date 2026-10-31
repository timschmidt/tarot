//! Read or change what Nura shows at boot. Changes run as root through pkexec.
//!
//! `status` prints JSON for anyone. `set tarot|greeter` requires root and a
//! PKEXEC_UID; direct boot is configured for that invoking account only.
use anyhow::{Context, Result, bail, ensure};
use tarot::boot_destination::{self, Paths};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let paths = Paths::default();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["status"] => {
            println!(
                "{}",
                serde_json::to_string(&boot_destination::status(&paths)?)?
            );
            Ok(())
        }
        ["set", choice] => {
            ensure!(
                rustix::process::geteuid().is_root(),
                "changing the boot destination requires administrator authorization"
            );
            let uid: u32 = std::env::var("PKEXEC_UID")
                .context("run through pkexec so the requesting account is known")?
                .parse()
                .context("invalid PKEXEC_UID")?;
            ensure!(uid != 0, "direct boot is for an ordinary account");
            match *choice {
                "tarot" => {
                    let user = boot_destination::account_name(
                        &std::fs::read_to_string("/etc/passwd")?,
                        uid,
                    )?;
                    boot_destination::select_tarot(&paths, &user)?;
                }
                "greeter" => boot_destination::select_greeter(&paths)?,
                _ => bail!("choose tarot or greeter"),
            }
            println!(
                "{}",
                serde_json::to_string(&boot_destination::status(&paths)?)?
            );
            Ok(())
        }
        _ => bail!("usage: tarot-boot-destination status | set tarot|greeter"),
    }
}
