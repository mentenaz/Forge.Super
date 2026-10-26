//! Unix domain sockets on Windows (AF_UNIX, Windows 10 1803 and later),
//! which Rust's standard library only offers on Unix. Blocking only: the
//! helper talks one short request at a time.
//!
//! Adapted from Zed's `net` crate (Zed Industries, Inc., GPL-3.0-or-later),
//! keeping only the blocking listener and stream.

use std::io::{Error, ErrorKind, Read, Result, Write};
use std::path::Path;
use std::sync::Once;

use windows::Win32::Foundation::{HANDLE, HANDLE_FLAG_INHERIT, HANDLE_FLAGS, SetHandleInformation};
use windows::Win32::Networking::WinSock::{
    ADDRESS_FAMILY, AF_UNIX, SEND_RECV_FLAGS, SOCK_STREAM, SOCKADDR_UN, SOCKET, SOCKET_ERROR,
    SOMAXCONN, WSAGetLastError, WSASocketW, WSAStartup, accept, bind, closesocket, connect, listen,
    recv, send,
};

/// A socket, closed when dropped. Not inherited by child processes.
struct Socket(SOCKET);

impl Socket {
    fn new() -> Result<Self> {
        init();
        unsafe {
            let raw = WSASocketW(AF_UNIX as _, SOCK_STREAM.0, 0, None, 0, Default::default())?;
            let socket = Self(raw);
            SetHandleInformation(
                HANDLE(raw.0 as _),
                HANDLE_FLAG_INHERIT.0,
                HANDLE_FLAGS::default(),
            )?;
            Ok(socket)
        }
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        unsafe { closesocket(self.0) };
    }
}

pub struct UnixListener(Socket);

impl UnixListener {
    pub fn bind<P: AsRef<Path>>(path: P) -> Result<Self> {
        let socket = Socket::new()?;
        let (addr, len) = sockaddr_un(path.as_ref())?;
        unsafe {
            check(bind(socket.0, &addr as *const _ as *const _, len as i32))?;
            check(listen(socket.0, SOMAXCONN as _))?;
        }
        Ok(Self(socket))
    }

    /// Waits for the next connection. The second value matches the shape of
    /// `std::os::unix::net::UnixListener::accept`.
    pub fn accept(&self) -> Result<(UnixStream, ())> {
        let mut storage = SOCKADDR_UN::default();
        let mut len = std::mem::size_of_val(&storage) as i32;
        let raw = unsafe {
            accept(
                self.0.0,
                Some(&mut storage as *mut _ as *mut _),
                Some(&mut len),
            )
        }?;
        Ok((UnixStream(Socket(raw)), ()))
    }
}

pub struct UnixStream(Socket);

impl UnixStream {
    pub fn connect<P: AsRef<Path>>(path: P) -> Result<Self> {
        let socket = Socket::new()?;
        let (addr, len) = sockaddr_un(path.as_ref())?;
        unsafe { check(connect(socket.0, &addr as *const _ as *const _, len as i32))? };
        Ok(Self(socket))
    }
}

impl Read for UnixStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        check(unsafe { recv(self.0.0, buf, SEND_RECV_FLAGS::default()) })
    }
}

impl Write for UnixStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        check(unsafe { send(self.0.0, buf, SEND_RECV_FLAGS::default()) })
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Starts Winsock, once per process.
fn init() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        let mut data = std::mem::zeroed();
        let result = WSAStartup(0x202, &mut data);
        assert!(result == 0, "WSAStartup failed: {result}");
    });
}

/// The address for `path`, and its length. See
/// https://devblogs.microsoft.com/commandline/af_unix-comes-to-windows/
fn sockaddr_un(path: &Path) -> Result<(SOCKADDR_UN, usize)> {
    let mut addr = SOCKADDR_UN {
        sun_family: ADDRESS_FAMILY(AF_UNIX),
        ..Default::default()
    };
    let bytes = path.to_str().ok_or(ErrorKind::InvalidInput)?.as_bytes();
    if bytes.contains(&0) {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "paths may not contain interior null bytes",
        ));
    }
    if bytes.len() >= addr.sun_path.len() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "path must be shorter than SUN_LEN",
        ));
    }
    for (to, from) in addr.sun_path.iter_mut().zip(bytes) {
        *to = *from as _;
    }
    let offset = std::mem::offset_of!(SOCKADDR_UN, sun_path);
    // The terminating zero counts.
    Ok((addr, offset + bytes.len() + 1))
}

/// A Winsock return value as a `Result`.
fn check(ret: i32) -> Result<usize> {
    if ret == SOCKET_ERROR {
        Err(Error::from_raw_os_error(unsafe { WSAGetLastError().0 }))
    } else {
        Ok(ret as usize)
    }
}
