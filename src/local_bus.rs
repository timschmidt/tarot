//! Cancellable local bus setup and frame limits before zbus allocation.
//! Keep zbus's own message decoder and receive sequence; do not reconstruct messages.
use anyhow::{Result, ensure};
use async_io::Async;
use rustix::net::{AddressFamily, SocketAddrUnix, SocketFlags, SocketType};
use std::{
    future::Future,
    io::{self, IoSliceMut},
    mem::MaybeUninit,
    net::Shutdown,
    os::{
        fd::{AsFd, BorrowedFd, OwnedFd},
        unix::net::UnixStream,
    },
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
    thread,
    time::{Duration, Instant},
};
use zbus::{
    address::{Transport, transport::UnixSocket},
    blocking::Connection,
    connection::socket::{ReadHalf, Split, WriteHalf},
};

pub const METHOD_TIMEOUT: Duration = Duration::from_millis(500);
const SETUP_TIMEOUT: Duration = Duration::from_secs(2);
const CANCEL_POLL: Duration = Duration::from_millis(10);
const MAX_AUTH: usize = 4096;
const MAX_LINE: usize = 1024;
const MAX_BODY: usize = 64 * 1024;
const MAX_FIELDS: usize = 16 * 1024;
const MAX_FDS: usize = 4;

#[derive(Debug)]
pub struct Cancellation {
    stop: Arc<AtomicBool>,
    socket: Mutex<Option<UnixStream>>,
}
impl Cancellation {
    pub fn new(stop: Arc<AtomicBool>) -> Self {
        Self {
            stop,
            socket: Mutex::new(None),
        }
    }
    pub fn cancel(&self) {
        self.stop.store(true, Ordering::Release);
        if let Some(socket) = self.socket.lock().unwrap().as_ref() {
            // Do not wait for zbus's async writer lock on the compositor thread.
            let _ = socket.shutdown(Shutdown::Both);
        }
    }
    fn install(&self, stream: &UnixStream) -> io::Result<()> {
        let mut socket = self.socket.lock().unwrap();
        if self.stop.load(Ordering::Acquire) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "menu connection retired",
            ));
        }
        *socket = Some(stream.try_clone()?);
        Ok(())
    }
}

#[derive(Debug)]
struct Guard {
    cancel: Arc<Cancellation>,
    ready: AtomicBool,
    deadline: Instant,
}
impl Guard {
    fn check(&self) -> io::Result<()> {
        if self.cancel.stop.load(Ordering::Acquire) {
            Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "menu connection retired",
            ))
        } else if !self.ready.load(Ordering::Acquire) && Instant::now() >= self.deadline {
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "menu bus setup deadline",
            ))
        } else {
            Ok(())
        }
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        self.cancel.socket.lock().unwrap().take();
    }
}
struct Notify(thread::Thread);
impl Wake for Notify {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}
fn complete<F: Future>(future: F, guard: &Guard) -> io::Result<F::Output> {
    let waker = Waker::from(Arc::new(Notify(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        guard.check()?;
        match future.as_mut().poll(&mut context) {
            Poll::Ready(result) => return Ok(result),
            Poll::Pending => thread::park_timeout(CANCEL_POLL),
        }
    }
}

pub fn connect(address: &str, cancel: Arc<Cancellation>) -> Result<Connection> {
    ensure!(address.len() <= 4096, "oversized local bus address");
    let guard = Arc::new(Guard {
        cancel,
        ready: AtomicBool::new(false),
        deadline: Instant::now() + SETUP_TIMEOUT,
    });
    guard.check()?;
    let address: zbus::Address = address.try_into()?;
    let Transport::Unix(unix) = address.transport() else {
        anyhow::bail!("native process credentials require a local Unix bus");
    };
    let target = match unix.path() {
        UnixSocket::File(path) => SocketAddrUnix::new(path)?,
        UnixSocket::Abstract(name) => SocketAddrUnix::new_abstract_name(name.as_encoded_bytes())?,
        _ => anyhow::bail!("native menu bus requires a concrete Unix endpoint"),
    };
    // zbus's address builder offloads Unix connect to a blocking pool. Own the
    // nonblocking descriptor here instead, including a full listener backlog.
    let fd = rustix::net::socket_with(
        AddressFamily::UNIX,
        SocketType::STREAM,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )?;
    loop {
        guard.check()?;
        match rustix::net::connect(&fd, &target) {
            Ok(()) | Err(rustix::io::Errno::ISCONN) => break,
            Err(
                rustix::io::Errno::AGAIN
                | rustix::io::Errno::INPROGRESS
                | rustix::io::Errno::ALREADY
                | rustix::io::Errno::INTR,
            ) => thread::park_timeout(CANCEL_POLL),
            Err(error) => return Err(error.into()),
        }
    }
    let stream = UnixStream::from(fd);
    guard.cancel.install(&stream)?;
    let socket = Arc::new(Async::new(stream)?);
    let split: zbus::connection::socket::BoxedSplit = Split::new(
        Box::new(Reader {
            socket: socket.clone(),
            guard: guard.clone(),
            line: vec![],
            auth_total: 0,
            authenticated: false,
            binary: false,
            header: [0; 16],
            header_pos: 0,
            remaining: 0,
            fds: 0,
        }),
        Box::new(Writer {
            socket,
            guard: guard.clone(),
        }),
    );
    let builder = zbus::connection::Builder::socket(split)
        .method_timeout(METHOD_TIMEOUT)
        .max_queued(16);
    let connection = complete(builder.build(), &guard)??;
    if let Some(guid) = address.guid() {
        ensure!(
            connection.server_guid().as_str() == guid.as_str(),
            "menu bus address GUID mismatch"
        );
    }
    guard.check()?;
    guard.ready.store(true, Ordering::Release);
    Ok(connection.into())
}

#[derive(Debug)]
struct Reader {
    socket: Arc<Async<UnixStream>>,
    guard: Arc<Guard>,
    line: Vec<u8>,
    auth_total: usize,
    authenticated: bool,
    binary: bool,
    header: [u8; 16],
    header_pos: usize,
    remaining: usize,
    fds: usize,
}
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "menu bus wire limit or invalid header",
    )
}
fn receive(stream: &UnixStream, buf: &mut [u8]) -> io::Result<(usize, Vec<OwnedFd>)> {
    use rustix::net::{RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags};
    // Limit descriptors at the syscall, and set CLOEXEC atomically so an app
    // launched concurrently cannot inherit a just-received process handle.
    // The ordinary cmsg_space! includes alignment slack, which could fit
    // additional descriptors. Give the kernel exactly four aligned slots.
    #[repr(align(8))]
    struct Control([MaybeUninit<u8>; rustix::cmsg_aligned_space!(ScmRights(MAX_FDS))]);
    let mut control =
        Control([MaybeUninit::uninit(); rustix::cmsg_aligned_space!(ScmRights(MAX_FDS))]);
    let mut ancillary = RecvAncillaryBuffer::new(&mut control.0);
    let message = rustix::net::recvmsg(
        stream.as_fd(),
        &mut [IoSliceMut::new(buf)],
        &mut ancillary,
        RecvFlags::CMSG_CLOEXEC,
    )?;
    let mut fds = Vec::with_capacity(MAX_FDS);
    for message in ancillary.drain() {
        match message {
            RecvAncillaryMessage::ScmRights(received) => fds.extend(received),
            RecvAncillaryMessage::ScmCredentials(_) => (),
            _ => return Err(invalid()),
        }
    }
    if message.flags.contains(ReturnFlags::CTRUNC) || fds.len() > MAX_FDS {
        return Err(invalid());
    }
    Ok((message.bytes, fds))
}
impl Reader {
    fn header_complete(&mut self) -> io::Result<()> {
        let word = |start| {
            let bytes = self.header[start..start + 4].try_into().unwrap();
            match self.header[0] {
                b'l' => Ok(u32::from_le_bytes(bytes) as usize),
                b'B' => Ok(u32::from_be_bytes(bytes) as usize),
                _ => Err(invalid()),
            }
        };
        let body = word(4)?;
        let fields = word(12)?;
        if self.header[3] != 1 || body > MAX_BODY || fields > MAX_FIELDS {
            return Err(invalid());
        }
        self.remaining = fields + ((8 - fields % 8) % 8) + body;
        Ok(())
    }
}
#[async_trait::async_trait]
impl ReadHalf for Reader {
    async fn recvmsg(&mut self, buf: &mut [u8]) -> io::Result<(usize, Vec<OwnedFd>)> {
        self.guard.check()?;
        if self.binary && self.header_pos == 16 && self.remaining == 0 {
            self.header_pos = 0;
            self.fds = 0;
        }
        // Authentication is read one byte at a time to separate the final
        // AGREE_UNIX_FD (or ERROR) line from an immediately following Hello.
        let limit = if !self.binary {
            1
        } else if self.header_pos < 16 {
            16 - self.header_pos
        } else {
            self.remaining
        };
        let limit = buf.len().min(limit);
        let (read, fds) = self
            .socket
            .read_with(|stream| {
                self.guard.check()?;
                receive(stream, &mut buf[..limit])
            })
            .await?;
        self.guard.check()?;
        if !self.binary {
            if !fds.is_empty() {
                return Err(invalid());
            }
            self.auth_total += read;
            self.line.extend_from_slice(&buf[..read]);
            if self.auth_total > MAX_AUTH || self.line.len() > MAX_LINE {
                return Err(invalid());
            }
            if self.line.ends_with(b"\n") {
                if !self.line.ends_with(b"\r\n") {
                    return Err(invalid());
                }
                if self.authenticated {
                    if !(self.line == b"AGREE_UNIX_FD\r\n" || self.line.starts_with(b"ERROR ")) {
                        return Err(invalid());
                    }
                    self.binary = true;
                } else if self.line.starts_with(b"OK ") {
                    self.authenticated = true;
                }
                self.line.clear();
            }
        } else {
            self.fds += fds.len();
            if self.fds > MAX_FDS {
                return Err(invalid());
            }
            if self.header_pos < 16 {
                self.header[self.header_pos..self.header_pos + read].copy_from_slice(&buf[..read]);
                self.header_pos += read;
                // Reject the final header bytes before zbus can resize its
                // buffer from the announced lengths. Preserve its own decoder.
                if self.header_pos == 16 {
                    self.header_complete()?;
                }
            } else {
                self.remaining -= read;
            }
        }
        Ok((read, fds))
    }
    fn can_pass_unix_fd(&self) -> bool {
        true
    }
    async fn peer_credentials(&mut self) -> io::Result<zbus::fdo::ConnectionCredentials> {
        ReadHalf::peer_credentials(&mut self.socket).await
    }
}
#[derive(Debug)]
struct Writer {
    socket: Arc<Async<UnixStream>>,
    guard: Arc<Guard>,
}
#[async_trait::async_trait]
impl WriteHalf for Writer {
    async fn sendmsg(&mut self, buf: &[u8], fds: &[BorrowedFd<'_>]) -> io::Result<usize> {
        self.guard.check()?;
        let sent = self.socket.sendmsg(buf, fds).await?;
        self.guard.check()?;
        Ok(sent)
    }
    async fn close(&mut self) -> io::Result<()> {
        self.socket.get_ref().shutdown(Shutdown::Both)
    }
    fn can_pass_unix_fd(&self) -> bool {
        true
    }
    async fn peer_credentials(&mut self) -> io::Result<zbus::fdo::ConnectionCredentials> {
        ReadHalf::peer_credentials(&mut self.socket).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, sync::mpsc};

    #[test]
    fn received_descriptors_are_close_on_exec_and_excess_are_rejected() {
        use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags};
        use std::{fs::File, io::IoSlice};
        let (receiver, sender) = UnixStream::pair().unwrap();
        let source = File::open("/dev/null").unwrap();
        let send = |count| {
            let fds = vec![source.as_fd(); count];
            let mut control = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(8))];
            let mut ancillary = SendAncillaryBuffer::new(&mut control);
            assert!(ancillary.push(SendAncillaryMessage::ScmRights(&fds)));
            rustix::net::sendmsg(
                sender.as_fd(),
                &[IoSlice::new(b"x")],
                &mut ancillary,
                SendFlags::empty(),
            )
            .unwrap();
        };
        send(1);
        let (_, received) = receive(&receiver, &mut [0u8; 1]).unwrap();
        assert_eq!(received.len(), 1);
        assert!(
            rustix::io::fcntl_getfd(&received[0])
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        drop(received);
        send(MAX_FDS);
        let (_, received) = receive(&receiver, &mut [0u8; 1]).unwrap();
        assert_eq!(received.len(), MAX_FDS);
        assert!(received.iter().all(|fd| {
            rustix::io::fcntl_getfd(fd)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        }));
        drop(received);
        send(8);
        assert_eq!(
            receive(&receiver, &mut [0u8; 1]).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn cancellation_wakes_a_full_socket_without_waiting_for_the_writer_lock() {
        let (stream, _peer) = UnixStream::pair().unwrap();
        stream.set_nonblocking(true).unwrap();
        let data = [0u8; 16384];
        loop {
            match (&stream).write(&data) {
                Ok(_) => (),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => panic!("fill private socket: {error}"),
            }
        }
        let cancel = Arc::new(Cancellation::new(Arc::new(AtomicBool::new(false))));
        cancel.install(&stream).unwrap();
        let guard = Arc::new(Guard {
            cancel: cancel.clone(),
            ready: AtomicBool::new(true),
            deadline: Instant::now(),
        });
        let writer = Arc::new(Mutex::new(Writer {
            socket: Arc::new(Async::new(stream).unwrap()),
            guard,
        }));
        let (started, entered) = mpsc::sync_channel(1);
        let (finished, done) = mpsc::sync_channel(1);
        let held = writer.clone();
        let worker = thread::spawn(move || {
            let mut locked = held.lock().unwrap();
            started.send(()).unwrap();
            // No setup timer drives this future. Only socket shutdown can
            // wake the blocked sender while it owns the writer lock.
            finished
                .send(async_io::block_on(locked.sendmsg(b"pending", &[])))
                .unwrap();
        });
        entered.recv_timeout(Duration::from_secs(1)).unwrap();
        thread::sleep(Duration::from_millis(20));
        assert!(writer.try_lock().is_err() && !worker.is_finished());
        let (stopped, stop_done) = mpsc::sync_channel(1);
        let shutdown = thread::spawn(move || {
            cancel.cancel();
            stopped.send(()).unwrap();
        });
        stop_done.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(done.recv_timeout(Duration::from_secs(1)).unwrap().is_err());
        shutdown.join().unwrap();
        worker.join().unwrap();
    }
}
