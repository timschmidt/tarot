//! elogind system-sleep hook for USB gadgets without an attached host.
//!
//! On the PinePhone Pro's kernel, suspending a bound dwc3 gadget with no host
//! attached fails its soft disconnect, and the controller aborts every system
//! sleep with -EAGAIN. Before sleep this unbinds only gadgets whose controller
//! reports "not attached", recording them first; after sleep it rebinds them.
//! Attached gadgets are never touched. Errors are logged, never fatal to sleep.
use anyhow::{Context, Result, bail, ensure};
use std::{
    fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::PathBuf,
};

const RECORD: &str = "tarot-usb-sleep";

struct Roots {
    gadgets: PathBuf,
    udc: PathBuf,
    run: PathBuf,
}

impl Roots {
    fn system() -> Self {
        Self {
            gadgets: "/sys/kernel/config/usb_gadget".into(),
            udc: "/sys/class/udc".into(),
            run: "/run".into(),
        }
    }
    fn record(&self) -> PathBuf {
        self.run.join(RECORD)
    }
}

/// Kernel names only: no separators, traversal, whitespace or control bytes.
fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|b| b.is_ascii_graphic() && b != b'/' && b != b'\\')
}

fn bound_udc(roots: &Roots, gadget: &str) -> Result<String> {
    let text = fs::read_to_string(roots.gadgets.join(gadget).join("UDC"))?;
    Ok(text.trim().to_owned())
}

/// Unbind detached gadgets; returns the recorded (gadget, controller) pairs.
fn before_sleep(roots: &Roots) -> Result<Vec<(String, String)>> {
    let record = roots.record();
    ensure!(
        !record.exists(),
        "an earlier sleep left {} unrestored",
        record.display()
    );
    let mut detached = Vec::new();
    let entries = match fs::read_dir(&roots.gadgets) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(detached),
        Err(error) => return Err(error).context("listing USB gadgets"),
    };
    for entry in entries {
        let entry = entry?;
        let Some(gadget) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !plain_name(&gadget) || !entry.file_type()?.is_dir() {
            continue;
        }
        let udc = bound_udc(roots, &gadget)?;
        if udc.is_empty() {
            continue;
        }
        ensure!(plain_name(&udc), "unexpected controller name for {gadget}");
        let state = fs::read_to_string(roots.udc.join(&udc).join("state"))?;
        if state.trim() == "not attached" {
            detached.push((gadget, udc));
        }
    }
    if detached.is_empty() {
        return Ok(detached);
    }
    // Record before unbinding so a failure part-way can still be restored.
    let text: String = detached
        .iter()
        .map(|(gadget, udc)| format!("{gadget}\t{udc}\n"))
        .collect();
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&record)
        .context("creating restore record")?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    for (gadget, _) in &detached {
        fs::write(roots.gadgets.join(gadget).join("UDC"), "\n")
            .with_context(|| format!("unbinding {gadget}"))?;
    }
    Ok(detached)
}

/// Rebind recorded gadgets that are still unbound; returns the restored count.
fn after_sleep(roots: &Roots) -> Result<usize> {
    let record = roots.record();
    let text = match fs::read_to_string(&record) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error).context("reading restore record"),
    };
    let mut restored = 0;
    let mut failure = None;
    for line in text.lines() {
        let Some((gadget, udc)) = line.split_once('\t') else {
            bail!("malformed restore record");
        };
        ensure!(
            plain_name(gadget) && plain_name(udc),
            "malformed restore record"
        );
        // Someone may have bound it deliberately while asleep; keep that choice.
        match bound_udc(roots, gadget) {
            Ok(current) if current.is_empty() => {
                match fs::write(roots.gadgets.join(gadget).join("UDC"), format!("{udc}\n")) {
                    Ok(()) => restored += 1,
                    Err(error) => failure = Some(format!("rebinding {gadget}: {error}")),
                }
            }
            Ok(_) => {}
            Err(error) => failure = Some(format!("reading {gadget}: {error}")),
        }
    }
    if let Some(failure) = failure {
        // Keep the record so a later post hook or operator can retry.
        bail!(failure);
    }
    fs::remove_file(&record)?;
    Ok(restored)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !rustix::process::geteuid().is_root() {
        eprintln!("tarot-usb-sleep: root only; nothing changed");
        return;
    }
    let roots = Roots::system();
    let _ = fs::DirBuilder::new().mode(0o755).create(&roots.run);
    let result = match args.first().map(String::as_str) {
        Some("pre") => {
            before_sleep(&roots).map(|d| format!("unbound {} detached gadget(s)", d.len()))
        }
        Some("post") => after_sleep(&roots).map(|n| format!("rebound {n} gadget(s)")),
        _ => Ok("ignored".into()),
    };
    // elogind sleeps regardless of hook status; report clearly in its log.
    match result {
        Ok(message) => eprintln!("tarot-usb-sleep {}: {message}", args.join(" ")),
        Err(error) => eprintln!("tarot-usb-sleep {}: {error:#}", args.join(" ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> (tempfile::TempDir, Roots) {
        let dir = tempfile::tempdir().unwrap();
        let roots = Roots {
            gadgets: dir.path().join("gadgets"),
            udc: dir.path().join("udc"),
            run: dir.path().join("run"),
        };
        for path in [&roots.gadgets, &roots.udc, &roots.run] {
            fs::create_dir(path).unwrap();
        }
        (dir, roots)
    }
    fn gadget(roots: &Roots, name: &str, udc: &str) {
        fs::create_dir(roots.gadgets.join(name)).unwrap();
        fs::write(roots.gadgets.join(name).join("UDC"), format!("{udc}\n")).unwrap();
    }
    fn controller(roots: &Roots, name: &str, state: &str) {
        fs::create_dir_all(roots.udc.join(name)).unwrap();
        fs::write(roots.udc.join(name).join("state"), format!("{state}\n")).unwrap();
    }
    fn udc(roots: &Roots, name: &str) -> String {
        bound_udc(roots, name).unwrap()
    }

    #[test]
    fn only_detached_gadgets_are_unbound_and_then_restored() {
        let (_dir, roots) = tree();
        gadget(&roots, "g1", "fe800000.usb");
        controller(&roots, "fe800000.usb", "not attached");
        gadget(&roots, "g2", "other.usb");
        controller(&roots, "other.usb", "configured");
        gadget(&roots, "idle", "");
        let detached = before_sleep(&roots).unwrap();
        assert_eq!(detached, [("g1".to_owned(), "fe800000.usb".to_owned())]);
        assert_eq!(udc(&roots, "g1"), "");
        assert_eq!(udc(&roots, "g2"), "other.usb");
        assert_eq!(
            fs::read_to_string(roots.record()).unwrap(),
            "g1\tfe800000.usb\n"
        );
        // A second pre without a post refuses rather than losing the record.
        assert!(before_sleep(&roots).is_err());
        assert_eq!(after_sleep(&roots).unwrap(), 1);
        assert_eq!(udc(&roots, "g1"), "fe800000.usb");
        assert!(!roots.record().exists());
        assert_eq!(after_sleep(&roots).unwrap(), 0);
    }

    #[test]
    fn attached_or_absent_gadgets_change_nothing() {
        let (_dir, roots) = tree();
        assert!(before_sleep(&roots).unwrap().is_empty());
        gadget(&roots, "g1", "fe800000.usb");
        controller(&roots, "fe800000.usb", "configured");
        assert!(before_sleep(&roots).unwrap().is_empty());
        assert!(!roots.record().exists());
        assert_eq!(udc(&roots, "g1"), "fe800000.usb");
        fs::remove_dir_all(&roots.gadgets).unwrap();
        assert!(before_sleep(&roots).unwrap().is_empty());
    }

    #[test]
    fn deliberate_rebinding_while_asleep_is_kept_and_bad_records_are_retained() {
        let (_dir, roots) = tree();
        gadget(&roots, "g1", "fe800000.usb");
        controller(&roots, "fe800000.usb", "not attached");
        before_sleep(&roots).unwrap();
        fs::write(roots.gadgets.join("g1/UDC"), "other.usb\n").unwrap();
        assert_eq!(after_sleep(&roots).unwrap(), 0);
        assert_eq!(udc(&roots, "g1"), "other.usb");
        for bad in ["g1 fe800000.usb\n", "../g1\tfe800000.usb\n", "g1\t\n"] {
            fs::write(roots.record(), bad).unwrap();
            assert!(after_sleep(&roots).is_err(), "{bad:?}");
            assert!(roots.record().exists());
        }
        fs::remove_file(roots.record()).unwrap();
        fs::write(roots.record(), "missing\tfe800000.usb\n").unwrap();
        assert!(after_sleep(&roots).is_err());
        assert!(roots.record().exists());
    }

    #[test]
    fn controller_names_are_validated_before_use() {
        for good in ["fe800000.usb", "g1", "musb-hdrc.0"] {
            assert!(plain_name(good));
        }
        for bad in ["", ".", "..", "a/b", "a b", "a\\b", "a\nb"] {
            assert!(!plain_name(bad), "{bad:?}");
        }
        let (_dir, roots) = tree();
        gadget(&roots, "g1", "../evil");
        assert!(before_sleep(&roots).is_err());
        assert!(!roots.record().exists());
    }
}
