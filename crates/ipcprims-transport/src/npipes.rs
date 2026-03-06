#[cfg(windows)]
use std::io::{Read, Write};
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::io::{FromRawHandle, RawHandle};
#[cfg(windows)]
use std::path::{Path, PathBuf};

#[cfg(windows)]
use tracing::debug;
#[cfg(windows)]
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, GetLastError,
    HANDLE, INVALID_HANDLE_VALUE,
};
#[cfg(windows)]
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE, OPEN_EXISTING,
    PIPE_ACCESS_DUPLEX,
};
#[cfg(windows)]
use windows_sys::Win32::System::Pipes::{
    CreateNamedPipeW, WaitNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn ConnectNamedPipe(
        hnamedpipe: HANDLE,
        lpoverlapped: *mut windows_sys::Win32::System::IO::OVERLAPPED,
    ) -> i32;
}

use crate::error::{Result, TransportError};
use crate::traits::IpcStream;

#[cfg(windows)]
fn to_wide_null(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(windows)]
fn normalize_pipe_name(path: &Path) -> String {
    let raw = path.as_os_str().to_string_lossy().to_string();
    if raw.starts_with(r"\\.\pipe\") {
        raw
    } else {
        format!(r"\\.\pipe\{raw}")
    }
}

/// Windows named-pipe stream.
#[cfg(windows)]
pub struct NamedPipeStream {
    file: std::fs::File,
}

#[cfg(windows)]
impl NamedPipeStream {
    fn connect_raw(path: impl AsRef<Path>) -> Result<Self> {
        let pipe_name = normalize_pipe_name(path.as_ref());
        let wide = to_wide_null(&pipe_name);

        // Retry loop for busy pipe instances and startup races where the
        // server has not yet created the first instance.
        for _ in 0..20 {
            // SAFETY: wide is NUL-terminated and valid for the duration of this call.
            let handle = unsafe {
                CreateFileW(
                    wide.as_ptr(),
                    FILE_GENERIC_READ | FILE_GENERIC_WRITE,
                    0,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    FILE_ATTRIBUTE_NORMAL,
                    std::ptr::null_mut(),
                )
            };

            if handle != INVALID_HANDLE_VALUE {
                debug!(pipe = %pipe_name, "connected to named pipe");
                // SAFETY: handle was returned by CreateFileW and is owned here.
                let file = unsafe { std::fs::File::from_raw_handle(handle as RawHandle) };
                return Ok(Self { file });
            }

            // SAFETY: GetLastError has no preconditions.
            let code = unsafe { GetLastError() };
            if code == ERROR_FILE_NOT_FOUND {
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
            }

            if code != ERROR_PIPE_BUSY {
                return Err(TransportError::Connect {
                    path: PathBuf::from(pipe_name),
                    source: std::io::Error::from_raw_os_error(code as i32),
                });
            }

            // SAFETY: wide is NUL-terminated; timeout is bounded.
            let waited = unsafe { WaitNamedPipeW(wide.as_ptr(), 5_000) };
            if waited == 0 {
                // SAFETY: GetLastError has no preconditions.
                let wait_code = unsafe { GetLastError() };
                return Err(TransportError::Connect {
                    path: PathBuf::from(pipe_name),
                    source: std::io::Error::from_raw_os_error(wait_code as i32),
                });
            }
        }

        Err(TransportError::Connect {
            path: PathBuf::from(pipe_name),
            source: std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "timed out waiting for named pipe server",
            ),
        })
    }

    pub(crate) fn try_clone(&self) -> Result<Self> {
        Ok(Self {
            file: self.file.try_clone()?,
        })
    }

    pub(crate) fn set_read_timeout(&self, _timeout: Option<std::time::Duration>) -> Result<()> {
        Ok(())
    }

    pub(crate) fn set_write_timeout(&self, _timeout: Option<std::time::Duration>) -> Result<()> {
        Ok(())
    }
}

#[cfg(windows)]
impl NamedPipeStream {
    /// Connect to a named pipe and return a generic IPC stream wrapper.
    pub fn connect(path: impl AsRef<Path>) -> Result<IpcStream> {
        let stream = Self::connect_raw(path)?;
        Ok(IpcStream::from_named_pipe(stream))
    }
}

#[cfg(windows)]
impl Read for NamedPipeStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.file.read(buf)
    }
}

#[cfg(windows)]
impl Write for NamedPipeStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.file.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

/// Windows named-pipe listener.
#[cfg(windows)]
pub struct NamedPipeListener {
    pipe_name: String,
}

#[cfg(windows)]
impl NamedPipeListener {
    pub fn bind(path: impl AsRef<Path>) -> Result<Self> {
        let pipe_name = normalize_pipe_name(path.as_ref());
        Ok(Self { pipe_name })
    }

    pub fn accept(&self) -> Result<IpcStream> {
        let wide = to_wide_null(&self.pipe_name);

        // SAFETY: wide is NUL-terminated and valid for call duration.
        let handle: HANDLE = unsafe {
            CreateNamedPipeW(
                wide.as_ptr(),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                64 * 1024,
                64 * 1024,
                0,
                std::ptr::null_mut(),
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            // SAFETY: GetLastError has no preconditions.
            let code = unsafe { GetLastError() };
            return Err(TransportError::Bind {
                path: PathBuf::from(&self.pipe_name),
                source: std::io::Error::from_raw_os_error(code as i32),
            });
        }

        // SAFETY: handle is valid if we reached here.
        let connected = unsafe { ConnectNamedPipe(handle, std::ptr::null_mut()) };
        if connected == 0 {
            // SAFETY: GetLastError has no preconditions.
            let code = unsafe { GetLastError() };
            if code != ERROR_PIPE_CONNECTED {
                // SAFETY: handle is valid and owned here.
                unsafe {
                    CloseHandle(handle);
                }
                return Err(TransportError::Accept(std::io::Error::from_raw_os_error(
                    code as i32,
                )));
            }
        }

        // SAFETY: handle is connected and ownership is transferred to File.
        let file = unsafe { std::fs::File::from_raw_handle(handle as RawHandle) };
        let stream = NamedPipeStream { file };
        Ok(IpcStream::from_named_pipe(stream))
    }

    pub fn path(&self) -> &Path {
        Path::new(&self.pipe_name)
    }
}

