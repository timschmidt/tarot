//! Bounded, nonblocking conversation with the standard Polkit PAM helper.
//!
//! Use on an authentication worker, never the renderer. A helper's SUCCESS is
//! meaningful only in the context of a live request from the verified authority;
//! this module cannot itself establish which caller is allowed to authenticate.
use anyhow::{Context, Result, bail, ensure};
use std::{
    ffi::CStr,
    io::{ErrorKind, Read, Write},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

pub const SYSTEM_HELPER: &str = "/usr/lib/polkit-1/polkit-agent-helper-1";
// Linux-PAM's helper reads with fgets(PAM_MAX_RESP_SIZE == 512). Leave room
// for the newline and terminator; a partial line could answer a later prompt.
pub use tarot_core::authorization::RESPONSE_MAX;
const LINE_MAX: usize = 8192;
const OUTPUT_MAX: usize = 65536;
const CONVERSATION_MAX: Duration = Duration::from_secs(120);

pub enum Event {
    Request { id: u64, prompt: String, echo: bool },
    Info(String),
    Error(String),
    Completed { authorized: bool },
}
impl std::fmt::Debug for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Request { id, echo, .. } => f
                .debug_struct("Request")
                .field("id", id)
                .field("echo", echo)
                .finish_non_exhaustive(),
            Self::Info(_) => f.write_str("Info([redacted])"),
            Self::Error(_) => f.write_str("Error([redacted])"),
            Self::Completed { authorized } => f
                .debug_struct("Completed")
                .field("authorized", authorized)
                .finish(),
        }
    }
}
struct PendingWrite {
    bytes: Zeroizing<Vec<u8>>,
    offset: usize,
}
/// Owns one child and its private pipes. Drop terminates and reaps that child.
/// Cookies/responses never appear in argv, environment, diagnostics or Debug.
pub struct Conversation {
    child: Child,
    input: Option<ChildStdin>,
    output: ChildStdout,
    write: Option<PendingWrite>,
    buffered: Zeroizing<Vec<u8>>,
    bytes_read: usize,
    request: u64,
    pending: Option<u64>,
    terminal: Option<bool>,
    completed: bool,
    eof: bool,
    deadline: Instant,
}
impl Conversation {
    /// Spawn the installed, root-owned, non-writable setuid helper for this UID.
    /// The caller must have already validated the authority/request/identity.
    pub fn system(uid: u32, cookie: Zeroizing<String>) -> Result<Self> {
        let path = Path::new(SYSTEM_HELPER);
        let canonical = path.canonicalize().context("Polkit helper unavailable")?;
        let metadata = canonical.metadata()?;
        ensure!(
            metadata.is_file() && metadata.uid() == 0 && metadata.mode() & 0o4022 == 0o4000,
            "Polkit helper has unsupported permissions"
        );
        for parent in canonical.ancestors().skip(1) {
            let m = parent.metadata()?;
            ensure!(
                m.uid() == 0 && m.permissions().mode() & 0o022 == 0,
                "Polkit helper directory is writable by another user"
            );
        }
        let name = username(uid)?;
        Self::spawn(Command::new(canonical), &name, cookie)
    }
    fn spawn(mut command: Command, username: &str, cookie: Zeroizing<String>) -> Result<Self> {
        ensure!(
            !cookie.is_empty() && cookie.len() <= 4096 && !cookie.chars().any(char::is_control),
            "Invalid authentication cookie"
        );
        command
            .arg(username)
            .env_clear()
            .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
            .env("LANG", "C.UTF-8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command
            .spawn()
            .context("Authentication helper could not start")?;
        let prepared = (|| -> Result<_> {
            let input = child.stdin.take().context("Missing authentication pipe")?;
            let output = child.stdout.take().context("Missing authentication pipe")?;
            for fd in [
                &input as &dyn std::os::fd::AsFd,
                &output as &dyn std::os::fd::AsFd,
            ] {
                let flags = rustix::fs::fcntl_getfl(fd)?;
                rustix::fs::fcntl_setfl(fd, flags | rustix::fs::OFlags::NONBLOCK)?;
            }
            Ok((input, output))
        })();
        let (input, output) = match prepared {
            Ok(pipes) => pipes,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        let mut bytes = Zeroizing::new(Vec::with_capacity(cookie.len() + 1));
        bytes.extend_from_slice(cookie.as_bytes());
        bytes.push(b'\n');
        Ok(Self {
            child,
            input: Some(input),
            output,
            write: Some(PendingWrite { bytes, offset: 0 }),
            buffered: Zeroizing::new(Vec::new()),
            bytes_read: 0,
            request: 0,
            pending: None,
            terminal: None,
            completed: false,
            eof: false,
            deadline: Instant::now() + CONVERSATION_MAX,
        })
    }
    pub fn pid(&self) -> u32 {
        self.child.id()
    }
    pub fn deadline(&self) -> Instant {
        self.deadline
    }
    /// Accept exactly one answer to the currently outstanding conversation turn.
    /// Invalid/late answers leave that turn intact; no input reaches another one.
    pub fn respond(&mut self, request: u64, response: &str) -> Result<()> {
        ensure!(
            !self.completed
                && self.terminal.is_none()
                && self.pending == Some(request)
                && Instant::now() < self.deadline,
            "Authentication question ended"
        );
        ensure!(
            self.write.is_none(),
            "Authentication input is still pending"
        );
        ensure!(
            response.len() <= RESPONSE_MAX && !response.chars().any(char::is_control),
            "Authentication response is too long or contains a control character"
        );
        let mut bytes = Zeroizing::new(Vec::with_capacity(response.len() + 1));
        bytes.extend_from_slice(response.as_bytes());
        bytes.push(b'\n');
        self.write = Some(PendingWrite { bytes, offset: 0 });
        self.pending = None;
        Ok(())
    }
    /// Advance available pipe I/O without waiting. The worker must arrange wakeups
    /// for pipe readiness, new responses, cancellation and the absolute deadline.
    pub fn advance(&mut self) -> Result<Vec<Event>> {
        if self.completed {
            return Ok(Vec::new());
        }
        let result = self.advance_inner();
        if result.is_err() {
            self.cancel();
        }
        result
    }
    fn advance_inner(&mut self) -> Result<Vec<Event>> {
        ensure!(
            Instant::now() < self.deadline,
            "Authentication conversation timed out"
        );
        if let Some(write) = &mut self.write {
            let input = self.input.as_mut().context("Authentication input closed")?;
            loop {
                match input.write(&write.bytes[write.offset..]) {
                    Ok(0) => bail!("Authentication input closed"),
                    Ok(count) => write.offset += count,
                    Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(_) => bail!("Authentication input failed"),
                }
                if write.offset == write.bytes.len() {
                    self.write = None;
                    break;
                }
            }
        }
        let mut scratch = Zeroizing::new([0u8; 4096]);
        loop {
            match self.output.read(&mut *scratch) {
                Ok(0) => {
                    self.eof = true;
                    break;
                }
                Ok(count) => {
                    self.bytes_read += count;
                    ensure!(
                        self.bytes_read <= OUTPUT_MAX,
                        "Authentication output limit exceeded"
                    );
                    self.buffered.extend_from_slice(&scratch[..count]);
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(_) => bail!("Authentication output failed"),
            }
        }
        let mut events = Vec::new();
        while let Some(end) = self.buffered.iter().position(|b| *b == b'\n') {
            ensure!(
                end <= LINE_MAX && self.terminal.is_none(),
                "Invalid authentication output"
            );
            let line = decode(&self.buffered[..end])?;
            self.buffered.drain(..=end);
            if line == "SUCCESS" || line == "FAILURE" {
                ensure!(
                    self.pending.is_none() && self.write.is_none(),
                    "Authentication ended during a question"
                );
                self.terminal = Some(line == "SUCCESS");
                self.input = None;
            } else if let Some(prompt) = line.strip_prefix("PAM_PROMPT_ECHO_OFF ") {
                events.push(self.question(prompt, false)?);
            } else if let Some(prompt) = line.strip_prefix("PAM_PROMPT_ECHO_ON ") {
                events.push(self.question(prompt, true)?);
            } else if let Some(info) = line.strip_prefix("PAM_TEXT_INFO ") {
                events.push(Event::Info(info.into()));
            } else if let Some(error) = line.strip_prefix("PAM_ERROR_MSG ") {
                events.push(Event::Error(error.into()));
            } else {
                bail!("Unrecognized authentication output");
            }
        }
        ensure!(
            self.buffered.len() <= LINE_MAX,
            "Authentication line limit exceeded"
        );
        if let Some(status) = self.child.try_wait()? {
            // Drain output before deciding completion: an exited child's final
            // bytes can become readable after this call's first read returned EAGAIN.
            if !self.eof {
                return Ok(events);
            }
            ensure!(self.buffered.is_empty(), "Truncated authentication output");
            let authorized = self
                .terminal
                .context("Authentication helper exited without a result")?;
            ensure!(
                !authorized || status.success(),
                "Authentication helper did not finish successfully"
            );
            self.completed = true;
            events.push(Event::Completed { authorized });
        }
        Ok(events)
    }
    fn question(&mut self, prompt: &str, echo: bool) -> Result<Event> {
        ensure!(
            self.pending.is_none() && self.write.is_none(),
            "Overlapping authentication questions"
        );
        self.request = self
            .request
            .checked_add(1)
            .context("Authentication question limit")?;
        ensure!(self.request <= 32, "Too many authentication questions");
        self.pending = Some(self.request);
        Ok(Event::Request {
            id: self.request,
            prompt: prompt.into(),
            echo,
        })
    }
    pub fn cancel(&mut self) {
        if self.completed {
            return;
        }
        self.completed = true;
        self.pending = None;
        self.write = None;
        self.buffered.clear();
        self.input = None;
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Drop for Conversation {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn username(uid: u32) -> Result<String> {
    let mut storage = vec![0u8; 16384];
    loop {
        let mut entry = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut found = std::ptr::null_mut();
        // SAFETY: getpwuid_r initializes entry and points its fields into this
        // live owned buffer. The account name is copied before that buffer dies.
        let code = unsafe {
            libc::getpwuid_r(
                uid,
                entry.as_mut_ptr(),
                storage.as_mut_ptr().cast(),
                storage.len(),
                &mut found,
            )
        };
        if code == libc::ERANGE && storage.len() < 131072 {
            storage.resize(storage.len() * 2, 0);
            continue;
        }
        ensure!(
            code == 0 && !found.is_null(),
            "Authentication identity is unavailable"
        );
        // SAFETY: successful getpwuid_r supplied the initialized passwd entry.
        let entry = unsafe { entry.assume_init() };
        ensure!(
            entry.pw_uid == uid && !entry.pw_name.is_null(),
            "Authentication identity changed"
        );
        // SAFETY: pw_name is a terminated string owned by storage until this scope ends.
        let name = unsafe { CStr::from_ptr(entry.pw_name) }
            .to_str()
            .context("Unsupported account name")?;
        ensure!(
            !name.is_empty() && name.len() <= 256 && !name.chars().any(char::is_control),
            "Unsupported account name"
        );
        return Ok(name.into());
    }
}
fn decode(input: &[u8]) -> Result<String> {
    let mut output = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        let byte = input[i];
        i += 1;
        if byte != b'\\' {
            output.push(byte);
            continue;
        }
        let escape = *input.get(i).context("Truncated authentication escape")?;
        i += 1;
        output.push(match escape {
            b'\\' | b'"' => escape,
            b'b' => 8,
            b'f' => 12,
            b'n' => b'\n',
            b'r' => b'\r',
            b't' => b'\t',
            b'v' => 11,
            b'0'..=b'7' => {
                let mut value = u16::from(escape - b'0');
                for _ in 0..2 {
                    if let Some(next @ b'0'..=b'7') = input.get(i) {
                        value = value * 8 + u16::from(next - b'0');
                        i += 1;
                    } else {
                        break;
                    }
                }
                ensure!(value <= 255, "Invalid authentication escape");
                value as u8
            }
            _ => bail!("Invalid authentication escape"),
        });
    }
    ensure!(!output.contains(&0), "Invalid authentication text");
    // FromUtf8Error owns the original bytes; do not retain helper conversation
    // text in an error that a caller may format with Debug.
    String::from_utf8(output).map_err(|_| anyhow::anyhow!("Authentication text is not UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(script: &str) -> Conversation {
        let mut command = Command::new("/usr/bin/python3");
        command.args(["-u", "-c", script]);
        Conversation::spawn(command, "qa-user", Zeroizing::new("qa-cookie".into())).unwrap()
    }
    fn await_event(c: &mut Conversation) -> Event {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let mut events = c.advance().unwrap();
            if !events.is_empty() {
                assert_eq!(events.len(), 1);
                return events.remove(0);
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    #[test]
    fn private_pipes_keep_cookie_off_argv_and_stale_answers_out_of_the_next_question() {
        let mut c = fixture(
            r#"import sys
assert sys.argv[1:] == ['qa-user']
assert sys.stdin.readline() == 'qa-cookie\n'
print('PAM_PROMPT_ECHO_OFF Password:')
assert sys.stdin.readline() == 'synthetic-one\n'
print('PAM_PROMPT_ECHO_ON Token:')
assert sys.stdin.readline() == '002345\n'
print('SUCCESS')
"#,
        );
        let Event::Request {
            id: first,
            echo: false,
            ..
        } = await_event(&mut c)
        else {
            panic!()
        };
        assert!(c.respond(first + 1, "wrong-turn").is_err());
        assert!(c.respond(first, "line\ninjection").is_err());
        assert!(c.respond(first, &"x".repeat(RESPONSE_MAX + 1)).is_err());
        c.respond(first, "synthetic-one").unwrap();
        let Event::Request {
            id: second,
            echo: true,
            ..
        } = await_event(&mut c)
        else {
            panic!()
        };
        assert!(c.respond(first, "stale-secret").is_err());
        c.respond(second, "002345").unwrap();
        assert!(matches!(
            await_event(&mut c),
            Event::Completed { authorized: true }
        ));
        assert!(c.advance().unwrap().is_empty());
    }
    #[test]
    fn malformed_or_unterminated_output_never_authenticates_and_reaps_child() {
        for script in [
            "print('SUCCESS extra')",
            "print('PAM_PROMPT_ECHO_OFF first');print('PAM_PROMPT_ECHO_OFF second')",
            "import sys;sys.stdout.write('SUCCESS')",
            "print('x'*8193)",
            "print('SUCCESS');raise SystemExit(1)",
        ] {
            let mut c = fixture(script);
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                match c.advance() {
                    Ok(events) => assert!(
                        !events
                            .iter()
                            .any(|e| matches!(e, Event::Completed { authorized: true }))
                    ),
                    Err(_) => break,
                }
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
            assert!(c.child.try_wait().unwrap().is_some());
        }
    }
    #[test]
    fn maximum_response_fits_one_pam_read_and_rejection_preserves_info_events() {
        let mut c = fixture(
            r#"import sys
assert sys.stdin.buffer.readline() == b'qa-cookie\n'
print('PAM_TEXT_INFO Caf\\303\\251')
print('PAM_PROMPT_ECHO_OFF Password:')
line = sys.stdin.buffer.readline(511)
assert line == b'x' * 510 + b'\n'
print('PAM_PROMPT_ECHO_ON Token:')
assert sys.stdin.buffer.readline() == b'002345\n'
print('PAM_ERROR_MSG Authentication rejected')
print('FAILURE')
raise SystemExit(1)
"#,
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut info = false;
        let mut rejected = false;
        loop {
            for event in c.advance().unwrap() {
                match event {
                    Event::Info(text) => {
                        assert_eq!(text, "Café");
                        info = true;
                    }
                    Event::Request { id: 1, .. } => {
                        c.respond(1, &"x".repeat(RESPONSE_MAX)).unwrap()
                    }
                    Event::Request { id: 2, .. } => c.respond(2, "002345").unwrap(),
                    Event::Error(text) => {
                        assert_eq!(text, "Authentication rejected");
                        rejected = true;
                    }
                    Event::Completed { authorized } => {
                        assert!(!authorized && info && rejected);
                        return;
                    }
                    _ => panic!("Unexpected conversation event"),
                }
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    #[test]
    fn cancellation_and_absolute_timeout_reap_only_the_owned_waiting_helper() {
        for timeout in [false, true] {
            let mut c = fixture(
                "import sys,time;sys.stdin.readline();print('PAM_PROMPT_ECHO_OFF Password:');time.sleep(30)",
            );
            let Event::Request { id, .. } = await_event(&mut c) else {
                panic!()
            };
            let start = Instant::now();
            if timeout {
                c.deadline = start;
                assert!(c.advance().is_err());
            } else {
                c.cancel();
            }
            assert!(start.elapsed() < Duration::from_secs(2));
            assert!(c.child.try_wait().unwrap().is_some());
            assert!(c.respond(id, "late-secret").is_err());
            assert!(c.advance().unwrap().is_empty());
        }
    }
    #[test]
    fn helper_escaped_unicode_prompts_preserve_meaning_but_not_debug_contents() {
        let prompt = decode(br"PAM_PROMPT_ECHO_OFF Caf\303\251:\n").unwrap();
        assert_eq!(prompt, "PAM_PROMPT_ECHO_OFF Café:\n");
        let event = Event::Request {
            id: 1,
            prompt,
            echo: false,
        };
        assert!(!format!("{event:?}").contains("Café"));
        for bad in [
            br"bad\".as_slice(),
            br"bad\q",
            br"bad\400",
            br"bad\000",
            &[255],
        ] {
            assert!(decode(bad).is_err());
        }
        let error = decode(&[b'p', b'r', b'i', b'v', b'a', b't', b'e', 255]).unwrap_err();
        assert!(error.downcast_ref::<std::string::FromUtf8Error>().is_none());
    }
}
