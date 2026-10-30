//! Retained Linux pidfs identity. Numeric PIDs, UIDs and cgroups do not authorize
//! an application export. Unsupported kernels fail this capability explicitly.
use std::{
    io,
    mem::{MaybeUninit, size_of_val},
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    os::unix::net::UnixStream,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Identity {
    device: u64,
    inode: u64,
}

#[derive(Debug)]
pub struct Process {
    fd: OwnedFd,
    identity: Identity,
}

impl Process {
    /// Capture the creator of an accepted ordinary client socket. A socketpair
    /// created before exec identifies its creator, not the eventual child;
    /// trusted locker/input-method connections deliberately do not use this.
    pub fn from_peer(stream: &UnixStream) -> io::Result<Self> {
        let mut raw = -1i32;
        let mut size = size_of_val(&raw) as libc::socklen_t;
        // SAFETY: a live socket and correctly sized initialized output storage.
        let result = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERPIDFD,
                (&mut raw as *mut i32).cast(),
                &mut size,
            )
        };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        if raw < 0 {
            return Err(io::Error::other("SO_PEERPIDFD returned no descriptor"));
        }
        // SAFETY: successful SO_PEERPIDFD transfers a newly owned descriptor.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        if size as usize != size_of_val(&raw) {
            return Err(io::Error::other("SO_PEERPIDFD returned an invalid size"));
        }
        // SAFETY: this newly owned descriptor accepts the close-on-exec flag.
        if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Self::from_fd(fd)
    }

    pub fn from_fd(fd: OwnedFd) -> io::Result<Self> {
        if usize::BITS != 64 {
            return Err(io::Error::other("pidfs identity requires 64-bit Linux"));
        }
        let mut fs = MaybeUninit::<libc::statfs>::uninit();
        let mut stat = MaybeUninit::<libc::stat>::uninit();
        // SAFETY: both functions initialize their correctly sized outputs.
        if unsafe { libc::fstatfs(fd.as_raw_fd(), fs.as_mut_ptr()) } < 0
            || unsafe { libc::fstat(fd.as_raw_fd(), stat.as_mut_ptr()) } < 0
        {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: both calls succeeded.
        let (fs, stat) = unsafe { (fs.assume_init(), stat.assume_init()) };
        if fs.f_type != 0x5049_4446 || stat.st_ino == 0 {
            return Err(io::Error::other("not a supported pidfs process descriptor"));
        }
        let process = Self {
            fd,
            identity: Identity {
                device: stat.st_dev,
                inode: stat.st_ino,
            },
        };
        if !process.is_live()? {
            return Err(io::Error::other("process has already exited"));
        }
        Ok(process)
    }

    pub fn is_live(&self) -> io::Result<bool> {
        let mut event = libc::pollfd {
            fd: self.fd.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one live descriptor, one initialized pollfd, no waiting.
        if unsafe { libc::poll(&mut event, 1, 0) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(event.revents == 0)
    }

    pub fn same_live_process(&self, other: &Self) -> io::Result<bool> {
        Ok(self.identity == other.identity && self.is_live()? && other.is_live()?)
    }
}
