//! Narrow Linux-PAM boundary. Only a disposable, same-user child calls PAM.
use anyhow::{Context, Result, ensure};
use libc::{c_char, c_int, c_void};
use std::{
    ffi::{CStr, CString},
    io::{Read, Write},
    os::unix::process::CommandExt,
    process::{Child, Command, Stdio},
    ptr,
    time::{Duration, Instant},
};
use zeroize::{Zeroize, Zeroizing};

pub const MAX_SECRET: usize = 512;
const PAM_SUCCESS: c_int = 0;
const PAM_CONV_ERR: c_int = 19;
const PAM_SILENT: c_int = 0x8000;
const PAM_DISALLOW_NULL_AUTHTOK: c_int = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Accepted,
    Rejected,
    Unavailable,
}
impl Outcome {
    fn code(self) -> i32 {
        match self {
            Self::Accepted => 0,
            Self::Rejected => 1,
            Self::Unavailable => 2,
        }
    }
}

/// Fixed-capacity UTF-8 buffer, including erasure of deleted bytes. Never Debug,
/// Clone or Serialize; the complete secret never enters egui's text/undo state.
pub struct Entry(Zeroizing<Vec<u8>>);
impl Default for Entry {
    fn default() -> Self {
        Self(Zeroizing::new(Vec::with_capacity(MAX_SECRET)))
    }
}
impl Entry {
    pub fn push(&mut self, text: &str) {
        if self.0.len() + text.len() <= MAX_SECRET && !text.chars().any(char::is_control) {
            self.0.extend_from_slice(text.as_bytes());
        }
    }
    pub fn backspace(&mut self) {
        let text = std::str::from_utf8(&self.0).expect("entry UTF-8 invariant");
        if let Some((at, _)) = text.char_indices().next_back() {
            self.0[at..].zeroize();
            self.0.truncate(at);
        }
    }
    pub fn clear(&mut self) {
        self.0.zeroize();
    }
    pub fn count(&self) -> usize {
        std::str::from_utf8(&self.0)
            .expect("entry UTF-8 invariant")
            .chars()
            .count()
    }
    fn bytes(&self) -> &[u8] {
        &self.0
    }
}

/// Disable core dumps and ordinary same-user ptrace before collecting a secret.
/// This is not a sandbox or a guarantee against root or already-privileged code.
pub fn protect_process() -> Result<()> {
    // SAFETY: these process-local calls use fixed Linux ABI arguments.
    unsafe {
        ensure!(
            libc::getuid() != 0
                && libc::getuid() == libc::geteuid()
                && libc::getgid() == libc::getegid(),
            "locker requires an unprivileged user"
        );
        let limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        ensure!(
            libc::setrlimit(libc::RLIMIT_CORE, &limit) == 0,
            "cannot disable core dumps"
        );
        ensure!(
            libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) == 0,
            "cannot disable dumpability"
        );
    }
    Ok(())
}
fn username() -> Result<CString> {
    let mut record: libc::passwd = unsafe { std::mem::zeroed() };
    let mut result = ptr::null_mut();
    let mut buffer = vec![0u8; 65536];
    // SAFETY: record and scratch storage live until we copy the returned name.
    let rc = unsafe {
        libc::getpwuid_r(
            libc::getuid(),
            &mut record,
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        )
    };
    ensure!(
        rc == 0 && !result.is_null() && !record.pw_name.is_null(),
        "cannot resolve session account"
    );
    // SAFETY: successful getpwuid_r supplies a NUL-terminated name in buffer.
    Ok(unsafe { CStr::from_ptr(record.pw_name) }.to_owned())
}

#[repr(C)]
struct Message {
    style: c_int,
    text: *const c_char,
}
#[repr(C)]
struct Response {
    text: *mut c_char,
    code: c_int,
}
#[repr(C)]
struct Conversation {
    callback:
        unsafe extern "C" fn(c_int, *mut *const Message, *mut *mut Response, *mut c_void) -> c_int,
    data: *mut c_void,
}
struct Answers<'a> {
    user: &'a CStr,
    password: &'a [u8],
    password_sent: bool,
    unsupported: bool,
}

unsafe extern "C" fn converse(
    count: c_int,
    messages: *mut *const Message,
    output: *mut *mut Response,
    data: *mut c_void,
) -> c_int {
    // SAFETY: PAM invokes this synchronously with its documented arrays and our
    // live Answers. Every allocation transferred to PAM uses libc's allocator.
    unsafe {
        if output.is_null() || data.is_null() || messages.is_null() || !(1..=32).contains(&count) {
            return PAM_CONV_ERR;
        }
        *output = ptr::null_mut();
        let answers = &mut *data.cast::<Answers<'_>>();
        let array =
            libc::calloc(count as usize, std::mem::size_of::<Response>()).cast::<Response>();
        if array.is_null() {
            return PAM_CONV_ERR;
        }
        for i in 0..count as usize {
            let msg = *messages.add(i);
            let bytes = if msg.is_null() {
                None
            } else {
                match (*msg).style {
                    1 if !answers.password_sent => {
                        answers.password_sent = true;
                        Some(answers.password)
                    } // ECHO_OFF
                    2 => Some(answers.user.to_bytes()), // ECHO_ON: username only
                    3 | 4 => Some(&[][..]), // ERROR_MSG/TEXT_INFO, never copy arbitrary module messages
                    _ => None,
                }
            };
            let Some(bytes) = bytes else {
                answers.unsupported = true;
                for j in 0..i {
                    let value = (*array.add(j)).text;
                    if !value.is_null() {
                        let len = libc::strlen(value);
                        std::slice::from_raw_parts_mut(value.cast::<u8>(), len).zeroize();
                        libc::free(value.cast());
                    }
                }
                libc::free(array.cast());
                return PAM_CONV_ERR;
            };
            if !bytes.is_empty() {
                let value = libc::malloc(bytes.len() + 1).cast::<c_char>();
                if value.is_null() {
                    for j in 0..i {
                        let value = (*array.add(j)).text;
                        if !value.is_null() {
                            let len = libc::strlen(value);
                            std::slice::from_raw_parts_mut(value.cast::<u8>(), len).zeroize();
                            libc::free(value.cast());
                        }
                    }
                    libc::free(array.cast());
                    return PAM_CONV_ERR;
                }
                ptr::copy_nonoverlapping(bytes.as_ptr(), value.cast(), bytes.len());
                *value.add(bytes.len()) = 0;
                (*array.add(i)).text = value;
            }
        }
        *output = array;
        PAM_SUCCESS
    }
}

fn pam(password: &[u8], service: &CStr, confdir: Option<&CStr>) -> Result<Outcome> {
    ensure!(
        !password.is_empty() && password.len() <= MAX_SECRET && !password.contains(&0),
        "invalid authentication input"
    );
    let user = username()?;
    // SAFETY: load only the standard Linux-PAM SONAME. Function signatures match
    // its public ABI; all symbols and handles stay within the Library lifetime.
    unsafe {
        let library = libloading::Library::new("libpam.so.0")?;
        type Start = unsafe extern "C" fn(
            *const c_char,
            *const c_char,
            *const Conversation,
            *mut *mut c_void,
        ) -> c_int;
        type StartDir = unsafe extern "C" fn(
            *const c_char,
            *const c_char,
            *const Conversation,
            *const c_char,
            *mut *mut c_void,
        ) -> c_int;
        type Check = unsafe extern "C" fn(*mut c_void, c_int) -> c_int;
        type End = unsafe extern "C" fn(*mut c_void, c_int) -> c_int;
        let start: libloading::Symbol<Start> = library.get(b"pam_start\0")?;
        let authenticate: libloading::Symbol<Check> = library.get(b"pam_authenticate\0")?;
        let account: libloading::Symbol<Check> = library.get(b"pam_acct_mgmt\0")?;
        let end: libloading::Symbol<End> = library.get(b"pam_end\0")?;
        let mut answers = Answers {
            user: &user,
            password,
            password_sent: false,
            unsupported: false,
        };
        let conversation = Conversation {
            callback: converse,
            data: (&mut answers as *mut Answers<'_>).cast(),
        };
        let mut handle = ptr::null_mut();
        let rc = if let Some(dir) = confdir {
            let start_dir: libloading::Symbol<StartDir> = library.get(b"pam_start_confdir\0")?;
            start_dir(
                service.as_ptr(),
                user.as_ptr(),
                &conversation,
                dir.as_ptr(),
                &mut handle,
            )
        } else {
            start(service.as_ptr(), user.as_ptr(), &conversation, &mut handle)
        };
        if rc != PAM_SUCCESS || handle.is_null() {
            return Ok(Outcome::Unavailable);
        }
        let mut result = authenticate(handle, PAM_SILENT | PAM_DISALLOW_NULL_AUTHTOK);
        if result == PAM_SUCCESS {
            result = account(handle, PAM_SILENT);
        }
        let ended = end(handle, result);
        #[cfg(test)]
        eprintln!(
            "PAM fixture result={result}, end={ended}, unsupported={}",
            answers.unsupported
        );
        Ok(if answers.unsupported || ended != PAM_SUCCESS {
            Outcome::Unavailable
        } else {
            match result {
                0 => Outcome::Accepted,
                6 | 7 | 8 | 10 | 11 => Outcome::Rejected,
                _ => Outcome::Unavailable,
            }
        })
    }
}

pub fn worker() -> i32 {
    let result = (|| -> Result<Outcome> {
        protect_process()?;
        let mut length = [0u8; 2];
        std::io::stdin().read_exact(&mut length)?;
        let length = u16::from_le_bytes(length) as usize;
        ensure!((1..=MAX_SECRET).contains(&length), "invalid input length");
        let mut password = Zeroizing::new(vec![0u8; length]);
        std::io::stdin().read_exact(&mut password)?;
        pam(&password, c"tarot-lock", None)
    })();
    result.unwrap_or(Outcome::Unavailable).code()
}

pub struct Attempt {
    child: Child,
    started: Instant,
}
impl Attempt {
    pub fn start(entry: &mut Entry) -> Result<Self> {
        let result = (|| {
            ensure!(!entry.bytes().is_empty(), "empty credential");
            let mut command = Command::new(std::env::current_exe()?);
            command
                .arg("--pam-worker")
                .env_remove("WAYLAND_SOCKET")
                .env_remove("WAYLAND_DISPLAY")
                .env_remove("DISPLAY")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .process_group(0);
            // SAFETY: only async-signal-safe Linux syscalls run between fork and
            // exec. Bind this same-user child to its exact living parent before
            // any password is sent; the recheck closes the parent-death race.
            let parent = unsafe { libc::getpid() };
            unsafe {
                command.pre_exec(move || {
                    if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    if libc::getppid() != parent {
                        return Err(std::io::Error::from_raw_os_error(libc::ECHILD));
                    }
                    Ok(())
                });
            }
            let mut child = command.spawn()?;
            let sent = (|| -> Result<()> {
                let mut pipe = child.stdin.take().context("authentication pipe")?;
                pipe.write_all(&(entry.bytes().len() as u16).to_le_bytes())?;
                pipe.write_all(entry.bytes())?;
                Ok(())
            })();
            if let Err(error) = sent {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
            Ok(Self {
                child,
                started: Instant::now(),
            })
        })();
        entry.clear();
        result
    }
    pub fn poll(&mut self) -> Result<Option<Outcome>> {
        if let Some(exit) = self.child.try_wait()? {
            return Ok(Some(match exit.code() {
                Some(0) => Outcome::Accepted,
                Some(1) => Outcome::Rejected,
                _ => Outcome::Unavailable,
            }));
        }
        if self.started.elapsed() > Duration::from_secs(15) {
            self.terminate();
            return Ok(Some(Outcome::Unavailable));
        }
        Ok(None)
    }
    fn terminate(&mut self) {
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            return;
        }
        // SAFETY: the unreaped owned child was placed in its own process group;
        // kill descendants (e.g. an outstanding PAM helper), then reap it.
        unsafe {
            libc::kill(-(self.child.id() as i32), libc::SIGKILL);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Drop for Attempt {
    fn drop(&mut self) {
        self.terminate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_entry_bounds_and_erases_deleted_utf8() {
        let mut e = Entry::default();
        e.push("aé🙂");
        e.backspace();
        assert_eq!(e.bytes(), "aé".as_bytes());
        // Deleted bytes lie in initialized spare allocation, inspect without
        // making them part of the live vector again.
        unsafe {
            assert_eq!(
                std::slice::from_raw_parts(e.0.as_ptr().add(e.0.len()), 4),
                [0; 4]
            );
        }
        e.push("\n");
        assert_eq!(e.count(), 2);
        e.push(&"x".repeat(MAX_SECRET));
        assert_eq!(e.count(), 2);
        e.clear();
        assert!(e.bytes().is_empty());
    }
    #[test]
    fn stalled_worker_times_out_and_is_reaped_without_accepting() {
        let child = Command::new("/bin/sh")
            .args(["-c", "sleep 60 & wait"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .unwrap();
        let mut attempt = Attempt {
            child,
            started: Instant::now() - Duration::from_secs(16),
        };
        assert_eq!(attempt.poll().unwrap(), Some(Outcome::Unavailable));
        assert!(attempt.child.try_wait().unwrap().is_some());
    }
    #[test]
    fn canceled_stopped_worker_is_killed_and_reaped_without_an_outcome() {
        let child = Command::new("/bin/sh")
            .args(["-c", "kill -STOP $$; exit 0"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .unwrap();
        let pid = child.id();
        let mut attempt = Some(Attempt {
            child,
            started: Instant::now(),
        });
        let deadline = Instant::now() + Duration::from_secs(2);
        while !std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .unwrap()
            .rsplit(')')
            .next()
            .unwrap()
            .trim_start()
            .starts_with('T')
        {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        drop(attempt.take());
        assert!(attempt.is_none());
        assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
    }

    #[test]
    fn conversation_rejects_reusing_password_for_a_second_factor() {
        let mut a = Answers {
            user: c"fixture",
            password: b"synthetic",
            password_sent: false,
            unsupported: false,
        };
        let m = Message {
            style: 1,
            text: c"Password".as_ptr(),
        };
        let mut messages = [&m as *const _, &m as *const _];
        let mut out = ptr::null_mut();
        let rc = unsafe {
            converse(
                2,
                messages.as_mut_ptr(),
                &mut out,
                (&mut a as *mut Answers<'_>).cast(),
            )
        };
        assert_eq!(rc, PAM_CONV_ERR);
        assert!(out.is_null() && a.unsupported);
    }
    #[test]
    #[ignore = "requires host Linux-PAM, permit/deny modules and audit access outside restricted sandbox"]
    fn actual_pam_requires_both_authentication_and_account_policy() {
        let dir = tempfile::tempdir().unwrap();
        let config = CString::new(dir.path().as_os_str().as_encoded_bytes()).unwrap();
        for (auth, account, accepted) in [
            ("pam_permit.so", "pam_permit.so", true),
            ("pam_deny.so", "pam_permit.so", false),
            ("pam_permit.so", "pam_deny.so", false),
        ] {
            std::fs::write(
                dir.path().join("fixture"),
                format!("auth required {auth}\naccount required {account}\n"),
            )
            .unwrap();
            let outcome = pam(b"synthetic", c"fixture", Some(&config)).unwrap();
            assert_eq!(outcome == Outcome::Accepted, accepted);
        }
    }
}
