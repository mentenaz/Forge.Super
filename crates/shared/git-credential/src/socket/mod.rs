#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(windows)]
pub use windows::{UnixListener, UnixStream};
