// SPDX-License-Identifier: MIT OR Apache-2.0
//! Linux 64-bit pidfs identity, deliberately without a numeric-PID fallback.
use std::{
    io,
    mem::{MaybeUninit, size_of_val},
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    os::unix::net::UnixStream,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    device: u64,
    inode: u64,
}

pub fn peer(stream: &UnixStream) -> io::Result<OwnedFd> {
    let mut raw = -1i32;
    let mut size = size_of_val(&raw) as libc::socklen_t;
    // SAFETY: getsockopt receives a live socket and correctly sized output.
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
    assert_eq!(size as usize, size_of_val(&raw));
    // SAFETY: a successful SO_PEERPIDFD returns a newly owned descriptor.
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    // SAFETY: setting close-on-exec on this owned descriptor is valid.
    if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(fd)
}

pub fn live(fd: &OwnedFd) -> io::Result<bool> {
    let mut event = libc::pollfd {
        fd: fd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: the pollfd array has exactly one initialized entry.
    let result = unsafe { libc::poll(&mut event, 1, 0) };
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(result == 0 && event.revents == 0)
    }
}

pub fn identity(fd: &OwnedFd) -> io::Result<Identity> {
    if usize::BITS != 64 {
        return Err(io::Error::other(
            "pidfs inode comparison requires 64-bit Linux",
        ));
    }
    let mut fs = MaybeUninit::<libc::statfs>::uninit();
    let mut stat = MaybeUninit::<libc::stat>::uninit();
    // SAFETY: both functions initialize their correctly sized outputs on success.
    if unsafe { libc::fstatfs(fd.as_raw_fd(), fs.as_mut_ptr()) } < 0
        || unsafe { libc::fstat(fd.as_raw_fd(), stat.as_mut_ptr()) } < 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the successful calls above initialized these values.
    let (fs, stat) = unsafe { (fs.assume_init(), stat.assume_init()) };
    if fs.f_type != 0x5049_4446 || stat.st_ino == 0 {
        return Err(io::Error::other("not a supported pidfs process descriptor"));
    }
    Ok(Identity {
        device: stat.st_dev,
        inode: stat.st_ino,
    })
}

pub fn same_live_process(left: &OwnedFd, right: &OwnedFd) -> io::Result<bool> {
    let equal = identity(left)? == identity(right)?;
    Ok(equal && live(left)? && live(right)?)
}
