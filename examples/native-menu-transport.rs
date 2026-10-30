//! Real GIO producer acceptance for the production Rust wire adapter.
use anyhow::{Result, ensure};
use clap::Parser;
use serde_json::{Value, json};
use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};
use tarot::{
    native_menu::transport::{
        Authority, Binding, Dispatch, Export, Item, Scalar, session_connection,
    },
    process_identity::Process,
};

#[derive(Parser)]
struct Options {
    #[arg(long)]
    fixture: PathBuf,
    #[arg(long)]
    output: PathBuf,
}
struct Producer {
    child: Child,
    input: ChildStdin,
    lines: mpsc::Receiver<String>,
    owner: String,
    peer: Arc<Process>,
    _socket: UnixStream,
    report: PathBuf,
}
impl Drop for Producer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Producer {
    fn start(fixture: &Path, output: &Path, name: &str, mode: &str) -> Result<Self> {
        let socket = output.join(format!("{name}.sock"));
        let listener = UnixListener::bind(&socket)?;
        listener.set_nonblocking(true)?;
        let report = output.join(format!("{name}-callbacks.jsonl"));
        let mut child = Command::new("python3")
            .arg(fixture)
            .arg("--peer")
            .arg(&socket)
            .arg("--report")
            .arg(&report)
            .arg("--mode")
            .arg(mode)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(File::create(output.join(format!("{name}.log")))?)
            .spawn()?;
        let input = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (send, lines) = mpsc::sync_channel(8);
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                if send.send(line.unwrap_or_default()).is_err() {
                    break;
                }
            }
        });
        let started = (|| -> Result<_> {
            let line = lines.recv_timeout(Duration::from_secs(5))?;
            let value: Value = serde_json::from_str(&line)?;
            ensure!(
                value["pid"] == child.id() && value["uid"] == rustix::process::getuid().as_raw()
            );
            let owner = value["owner"].as_str().unwrap().to_owned();
            let (stream, _) = listener.accept()?;
            let peer = Arc::new(Process::from_peer(&stream)?);
            Ok((owner, stream, peer))
        })();
        std::fs::remove_file(socket)?;
        match started {
            Ok((owner, stream, peer)) => Ok(Self {
                child,
                input,
                lines,
                owner,
                peer,
                _socket: stream,
                report,
            }),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(error)
            }
        }
    }
    fn binding(&self, authority: &Authority) -> Result<Binding> {
        Binding::new(
            session_connection()?,
            self.export()?,
            self.peer.clone(),
            authority.lease(),
        )
    }
    fn export(&self) -> Result<Export> {
        Export::new(
            &self.owner,
            "/org/tarot/NativeMenu/menu",
            Some("/org/tarot/NativeMenu/app"),
            Some("/org/tarot/NativeMenu/window"),
        )
    }
    fn control(&mut self, value: &str) -> Result<()> {
        writeln!(self.input, "{value}")?;
        self.input.flush()?;
        let value: Value = serde_json::from_str(&self.lines.recv_timeout(Duration::from_secs(5))?)?;
        if let Some(owner) = value["owner"].as_str() {
            self.owner = owner.to_owned();
        }
        Ok(())
    }
    fn events(&self) -> Result<Vec<Value>> {
        let text = std::fs::read_to_string(&self.report)?;
        Ok(text
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()?)
    }
}
fn run(options: &Options) -> Result<Value> {
    std::fs::create_dir(&options.output)?;
    let authority = Authority::default();
    let mut left = Producer::start(&options.fixture, &options.output, "left", "normal")?;
    let right = Producer::start(&options.fixture, &options.output, "right", "normal")?;
    ensure!(!left.peer.same_live_process(&right.peer)?);
    ensure!(Export::new("org.tarot.Alias", "/menu", None, None).is_err());
    ensure!(
        Binding::new(
            session_connection()?,
            right.export()?,
            left.peer.clone(),
            authority.lease()
        )
        .is_err()
    );
    let binding = left.binding(&authority)?;
    let snapshot = binding.snapshot()?;
    ensure!(
        matches!(&snapshot.items[0], Item::Submenu { label, items } if label == "Preferences" && items.len() == 4)
    );
    ensure!(
        matches!(&snapshot.items[1], Item::Section { label, items } if label.as_deref() == Some("Other") && items.len() == 1)
    );
    ensure!(binding.activate(&snapshot, &[0, 2]).is_err());
    ensure!(binding.activate(&snapshot, &[0, 1])? == Dispatch::Replied);
    ensure!(binding.activate(&snapshot, &[0, 3])? == Dispatch::Replied);
    ensure!(binding.activate(&snapshot, &[0, 0])? == Dispatch::Replied);
    let checked = binding.snapshot()?;
    ensure!(
        matches!(&checked.items[0], Item::Submenu { items, .. } if matches!(&items[0], Item::Action { action, .. } if action.state == Some(Scalar::Bool(true))))
    );
    ensure!(
        left.events()?
            == vec![
                json!({"action":"choose","value":"night"}),
                json!({"action":"window","value":7}),
                json!({"action":"toggle","value":null})
            ]
    );
    left.control("disable")?;
    ensure!(binding.activate(&checked, &[0, 1]).is_err());
    let disabled = binding.snapshot()?;
    ensure!(binding.activate(&disabled, &[0, 1]).is_err());
    left.control("relabel")?;
    ensure!(binding.activate(&disabled, &[0, 0]).is_err());
    let renamed = binding.snapshot()?;
    let unrelated = right.binding(&authority)?;
    ensure!(unrelated.activate(&renamed, &[0]).is_err());
    let lease_authority = Authority::default();
    let retired = Binding::new(
        session_connection()?,
        left.export()?,
        left.peer.clone(),
        authority.lease().intersect(&lease_authority.lease())?,
    )?;
    let retired_snapshot = retired.snapshot()?;
    drop(lease_authority);
    ensure!(retired.activate(&retired_snapshot, &[0]).is_err());
    ensure!(left.events()?.len() == 3 && right.events()?.is_empty());
    left.control("reconnect")?;
    ensure!(binding.snapshot().is_err());
    let replacement = left.binding(&authority)?;
    let replacement_snapshot = replacement.snapshot()?;
    ensure!(replacement.activate(&replacement_snapshot, &[0, 1])? == Dispatch::Replied);
    left.control("slow")?;
    let before_timeout = replacement.snapshot()?;
    let started = Instant::now();
    ensure!(replacement.activate(&before_timeout, &[0, 1])? == Dispatch::Uncertain);
    ensure!(started.elapsed() < Duration::from_secs(2));
    let deadline = Instant::now() + Duration::from_secs(3);
    while left.events()?.len() < 5 {
        ensure!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    }
    ensure!(left.events()?.len() == 5);
    left.control("close")?;
    ensure!(replacement.snapshot().is_err() && left.peer.is_live()?);
    left.control("exit")?;
    ensure!(left.child.wait()?.success());
    ensure!(!left.peer.is_live()? && replacement.snapshot().is_err());
    let pending = Producer::start(&options.fixture, &options.output, "pending", "slow-load")?;
    let pending_authority = Authority::default();
    let pending_binding = pending.binding(&pending_authority)?;
    let worker = thread::spawn(move || pending_binding.snapshot());
    let deadline = Instant::now() + Duration::from_secs(3);
    while pending.events()?.is_empty() {
        ensure!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(2));
    }
    pending_authority.retire();
    ensure!(worker.join().expect("snapshot worker panicked").is_err());
    ensure!(pending.events()? == vec![json!({"stage":"Start entered"})]);
    let unsupported = Producer::start(
        &options.fixture,
        &options.output,
        "unsupported",
        "unsupported",
    )?;
    let unsupported_binding = unsupported.binding(&authority)?;
    let unsupported_snapshot = unsupported_binding.snapshot()?;
    ensure!(
        matches!(&unsupported_snapshot.items[0], Item::Submenu { items, .. }
        if matches!(&items[4], Item::Action { action, .. } if !action.supported))
    );
    ensure!(
        unsupported_binding
            .activate(&unsupported_snapshot, &[0, 4])
            .is_err()
    );
    ensure!(unsupported.events()?.is_empty());
    let mut rejected = Vec::new();
    for mode in [
        "many-actions",
        "short-link",
        "wrong-link",
        "cycle",
        "deep",
        "groups",
        "expanded",
        "oversized",
        "long-label",
        "many-items",
        "attributes",
        "total-bytes",
        "missing-root",
        "fd",
        "silent",
    ] {
        let producer = Producer::start(&options.fixture, &options.output, mode, mode)?;
        let binding = producer.binding(&authority)?;
        let started = Instant::now();
        ensure!(
            binding.snapshot().is_err(),
            "accepted malformed producer {mode}"
        );
        ensure!(started.elapsed() < Duration::from_secs(3));
        ensure!(producer.events()?.is_empty());
        rejected.push(mode);
    }
    ensure!(unrelated.snapshot().is_ok());
    authority.retire();
    ensure!(unrelated.snapshot().is_err());
    ensure!(Process::from_fd(File::open("/dev/null")?.into()).is_err());
    Ok(
        json!({"passed":true,"wrong_process_rejected":true,"typed_targets_and_state":true,"disabled_and_stale_choices_rejected":true,"retired_claim_and_foreign_binding_rejected":true,"retirement_during_pending_load":true,"composed_client_claim_authorities":true,"unsupported_typed_target_disabled":true,"same_process_connection_replacement":true,"process_exit":true,"uncertain_timeout_executes_later_without_retry":true,"actual_callbacks":left.events()?,"malformed_producers_rejected":rejected,"phone_ui_tested":false}),
    )
}
fn main() -> Result<()> {
    let options = Options::parse();
    let result = run(&options);
    match &result {
        Ok(report) => {
            std::fs::write(
                options.output.join("report.json"),
                serde_json::to_vec_pretty(report)?,
            )?;
            println!("{}", serde_json::to_string(report)?);
        }
        Err(error) => eprintln!("native menu adapter acceptance failed: {error:#}"),
    }
    result.map(|_| ())
}
