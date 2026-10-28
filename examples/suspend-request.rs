//! Root-only spare-phone QA observer. Keeps its own delay FD until the actual
//! compositor has completed protection, then lets normal elogind suspend run.
//! The parent test must already own a future RTC wake alarm.
use anyhow::{Context, Result, ensure};
use clap::Parser;
use serde_json::{Value, json};
use std::{
    fs::OpenOptions,
    io::{BufRead, BufReader, Write},
    os::unix::{
        fs::{FileTypeExt, MetadataExt, OpenOptionsExt},
        net::UnixStream,
    },
    path::PathBuf,
    time::{Duration, Instant},
};
use zbus::blocking::{Proxy, connection::Builder};

#[derive(Parser)]
struct Options {
    #[arg(long)]
    control_socket: PathBuf,
    #[arg(long)]
    pid: u32,
    #[arg(long)]
    report: PathBuf,
}
fn status(options: &Options) -> Result<Value> {
    let mut stream = UnixStream::connect(&options.control_socket)?;
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    stream.write_all(b"{\"command\":\"status\"}\n")?;
    let mut text = String::new();
    BufReader::new(stream).read_line(&mut text)?;
    let value: Value = serde_json::from_str(&text)?;
    ensure!(
        value["ok"] == true && value["pid"] == options.pid,
        "Unexpected compositor identity"
    );
    Ok(value)
}
fn counter(name: &str) -> Result<u64> {
    Ok(
        std::fs::read_to_string(format!("/sys/power/suspend_stats/{name}"))?
            .trim()
            .parse()?,
    )
}
fn boottime() -> Result<Duration> {
    Ok(rustix::time::clock_gettime(rustix::time::ClockId::Boottime).try_into()?)
}
fn main() -> Result<()> {
    ensure!(
        rustix::process::getuid().as_raw() == 0,
        "Root-only bounded spare-phone test"
    );
    ensure!(
        std::fs::read_to_string("/proc/device-tree/model")?.contains("PinePhone Pro"),
        "Restricted to verified spare"
    );
    let options = Options::parse();
    ensure!(
        options.control_socket == std::path::Path::new("/run/user/10000/tarot-drm.sock")
            || options.control_socket
                == std::path::Path::new("/run/user/10000/tarot-session/control.sock"),
        "Expected the bounded development or installed diagnostic socket"
    );
    let socket = std::fs::symlink_metadata(&options.control_socket)?;
    ensure!(
        socket.file_type().is_socket() && socket.uid() == 10000,
        "Expected the verified user's diagnostic Unix socket"
    );
    let alarm: u64 = std::fs::read_to_string("/sys/class/rtc/rtc0/wakealarm")?
        .trim()
        .parse()
        .context("Parent must own a future RTC alarm")?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    ensure!(
        alarm > now + 3 && alarm <= now + 35,
        "RTC wake deadline outside test bound"
    );
    let initial = status(&options)?;
    let before = initial["lock"]["sleep"]["gate"]["protected_preparations"]
        .as_u64()
        .context("No sleep coordinator")?;
    ensure!(
        initial["lock"]["phase"] == "Unlocked" && initial["lock"]["sleep"]["armed"] == true,
        "Need awake unprotected session with armed coordinator"
    );
    let success = counter("success")?;
    let failed = counter("fail")?;
    let connection = Builder::system()?
        .method_timeout(Duration::from_secs(10))
        .build()?;
    let manager = Proxy::new(
        &connection,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
    )?;
    let maximum: u64 = manager.get_property("InhibitDelayMaxUSec")?;
    ensure!(
        maximum >= 4_000_000,
        "Observer needs at least four seconds of delay budget"
    );
    let fd: zbus::zvariant::OwnedFd = manager.call(
        "Inhibit",
        &(
            "sleep",
            "Tarot suspend replay",
            "Observe protected KMS before kernel sleep",
            "delay",
        ),
    )?;
    rustix::io::fcntl_setfd(&fd, rustix::io::FdFlags::CLOEXEC)?;
    let mut report = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&options.report)?;
    let request_connection = connection.clone();
    let requester = std::thread::spawn(move || -> Result<()> {
        let manager = Proxy::new(
            &request_connection,
            "org.freedesktop.login1",
            "/org/freedesktop/login1",
            "org.freedesktop.login1.Manager",
        )?;
        let _: () = manager.call("Suspend", &(false,))?;
        Ok(())
    });
    let started = Instant::now();
    let started_boot = boottime()?;
    let observed = (|| -> Result<Value> {
        loop {
            let current = status(&options)?;
            ensure!(
                boottime()?.saturating_sub(started_boot) < Duration::from_secs(2),
                "Protected KMS preparation not observed within bounded delay"
            );
            let sleep = &current["lock"]["sleep"];
            if sleep["gate"]["preparing"] == true
                && sleep["gate"]["protected_preparations"]
                    .as_u64()
                    .is_some_and(|n| n > before)
                && sleep["armed"] == false
                && current["lock"]["phase"] != "Unlocked"
                && current["lock"]["last_presented_generation"] == current["lock"]["generation"]
            {
                return Ok(current);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    })();
    let proof = json!({"event":"protection-observed-before-observer-release","observed":observed.as_ref().ok(),"observation_error":observed.as_ref().err().map(ToString::to_string),"elapsed_us":started.elapsed().as_micros(),"elapsed_boottime_us":boottime()?.saturating_sub(started_boot).as_micros(),"delay_max_us":maximum,"rtc_deadline":alarm});
    writeln!(report, "{proof}")?;
    report.flush()?;
    // No kernel suspend can start before our release within this sub-two-second
    // observation (the verified manager delay maximum is at least four seconds).
    drop(fd);
    let request_result = requester
        .join()
        .map_err(|_| anyhow::anyhow!("Suspend request thread panicked"))?;
    let deadline = Instant::now() + Duration::from_secs(30);
    while counter("success")? == success && counter("fail")? == failed && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(20));
    }
    let after_success = counter("success")?;
    let after_fail = counter("fail")?;
    writeln!(
        report,
        "{}",
        json!({"event":"kernel-settled","success_before":success,"success_after":after_success,"fail_before":failed,"fail_after":after_fail,"request_error":request_result.as_ref().err().map(ToString::to_string)})
    )?;
    report.flush()?;
    request_result?;
    observed?;
    ensure!(
        after_success == success + 1 && after_fail == failed,
        "Kernel did not complete exactly one successful suspend cycle"
    );
    Ok(())
}
