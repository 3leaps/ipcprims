use std::io::{Read, Write};

use crate::error::Result;
#[cfg(windows)]
use crate::npipes::NamedPipeStream;

/// A connected IPC stream — implements Read + Write.
///
/// This is the fundamental I/O type returned by transport operations.
/// On Unix, this wraps a Unix domain socket stream.
/// On Windows, this wraps a named pipe handle.
pub struct IpcStream {
    inner: IpcStreamInner,
}

#[cfg_attr(not(unix), allow(dead_code))]
enum IpcStreamInner {
    #[cfg(unix)]
    Unix(std::os::unix::net::UnixStream),
    #[cfg(windows)]
    NamedPipe(NamedPipeStream),
}

#[cfg_attr(not(unix), allow(unused_variables))]
impl Read for IpcStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match &mut self.inner {
            #[cfg(unix)]
            IpcStreamInner::Unix(stream) => stream.read(buf),
            #[cfg(windows)]
            IpcStreamInner::NamedPipe(stream) => stream.read(buf),
        }
    }
}

#[cfg_attr(not(unix), allow(unused_variables))]
impl Write for IpcStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match &mut self.inner {
            #[cfg(unix)]
            IpcStreamInner::Unix(stream) => stream.write(buf),
            #[cfg(windows)]
            IpcStreamInner::NamedPipe(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match &mut self.inner {
            #[cfg(unix)]
            IpcStreamInner::Unix(stream) => stream.flush(),
            #[cfg(windows)]
            IpcStreamInner::NamedPipe(stream) => stream.flush(),
        }
    }
}

#[cfg_attr(not(unix), allow(unused_variables))]
impl IpcStream {
    /// Create an IpcStream from a Unix domain socket stream.
    #[cfg(unix)]
    pub(crate) fn from_unix(stream: std::os::unix::net::UnixStream) -> Self {
        Self {
            inner: IpcStreamInner::Unix(stream),
        }
    }

    /// Create an IpcStream from a Windows named pipe stream.
    #[cfg(windows)]
    pub(crate) fn from_named_pipe(stream: NamedPipeStream) -> Self {
        Self {
            inner: IpcStreamInner::NamedPipe(stream),
        }
    }

    /// Set read timeout on the underlying stream.
    pub fn set_read_timeout(&self, timeout: Option<std::time::Duration>) -> Result<()> {
        match &self.inner {
            #[cfg(unix)]
            IpcStreamInner::Unix(stream) => stream.set_read_timeout(timeout).map_err(Into::into),
            #[cfg(windows)]
            IpcStreamInner::NamedPipe(stream) => stream.set_read_timeout(timeout),
        }
    }

    /// Set write timeout on the underlying stream.
    pub fn set_write_timeout(&self, timeout: Option<std::time::Duration>) -> Result<()> {
        match &self.inner {
            #[cfg(unix)]
            IpcStreamInner::Unix(stream) => stream.set_write_timeout(timeout).map_err(Into::into),
            #[cfg(windows)]
            IpcStreamInner::NamedPipe(stream) => stream.set_write_timeout(timeout),
        }
    }

    /// Try to clone this stream (creates a new file descriptor).
    pub fn try_clone(&self) -> Result<Self> {
        match &self.inner {
            #[cfg(unix)]
            IpcStreamInner::Unix(stream) => {
                let cloned = stream.try_clone()?;
                Ok(Self::from_unix(cloned))
            }
            #[cfg(windows)]
            IpcStreamInner::NamedPipe(stream) => {
                let cloned = stream.try_clone()?;
                Ok(Self::from_named_pipe(cloned))
            }
        }
    }

    /// Get the credentials of the connected peer (Linux only).
    ///
    /// Returns `(uid, gid, pid)` via `SO_PEERCRED`, or `None` if unavailable.
    #[cfg(target_os = "linux")]
    pub fn peer_credentials(&self) -> Option<(u32, u32, u32)> {
        use std::os::fd::AsRawFd;

        let fd = match &self.inner {
            IpcStreamInner::Unix(stream) => stream.as_raw_fd(),
        };

        let mut cred = libc::ucred {
            pid: 0,
            uid: 0,
            gid: 0,
        };
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;

        // SAFETY: `cred` and `len` are valid writable pointers for the provided sizes,
        // and `fd` is an open Unix socket descriptor owned by this process.
        let rc = unsafe {
            libc::getsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut cred as *mut libc::ucred).cast::<libc::c_void>(),
                &mut len,
            )
        };

        if rc == 0 && len as usize == std::mem::size_of::<libc::ucred>() {
            Some((cred.uid, cred.gid, cred.pid as u32))
        } else {
            None
        }
    }

    /// Get the credentials of the connected peer.
    ///
    /// Returns `None` on platforms that do not expose peer credentials.
    #[cfg(not(target_os = "linux"))]
    pub fn peer_credentials(&self) -> Option<(u32, u32, u32)> {
        None
    }
}

#[cfg_attr(not(unix), allow(unused_variables))]
impl std::fmt::Debug for IpcStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.inner {
            #[cfg(unix)]
            IpcStreamInner::Unix(_) => f.debug_struct("IpcStream").field("type", &"unix").finish(),
            #[cfg(windows)]
            IpcStreamInner::NamedPipe(_) => {
                f.debug_struct("IpcStream").field("type", &"named-pipe").finish()
            }
        }
    }
}
