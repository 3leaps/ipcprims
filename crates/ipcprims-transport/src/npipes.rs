#[cfg(windows)]
use std::io::{Read, Write};
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle};
#[cfg(windows)]
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::sync::atomic::{AtomicU32, Ordering};

#[cfg(windows)]
use tracing::debug;
#[cfg(windows)]
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_IO_PENDING, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED,
    GetLastError, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
#[cfg(windows)]
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, WriteFile, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ,
    FILE_GENERIC_WRITE, FILE_FLAG_OVERLAPPED, OPEN_EXISTING, PIPE_ACCESS_DUPLEX,
};
#[cfg(windows)]
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
#[cfg(windows)]
use windows_sys::Win32::System::Threading::{CreateEventW, WaitForSingleObject, INFINITE};
#[cfg(windows)]
use windows_sys::core::BOOL;

#[cfg(windows)]
const MAX_TIMEOUT_MS: u32 = INFINITE - 1;

#[cfg(windows)]
struct EventHandle(HANDLE);

#[cfg(windows)]
impl EventHandle {
    fn create_manual_reset() -> std::io::Result<Self> {
        // SAFETY: null security attributes and name are valid; booleans are explicit.
        let handle = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
        if handle.is_null() {
            // SAFETY: GetLastError has no preconditions.
            let code = unsafe { GetLastError() };
            return Err(std::io::Error::from_raw_os_error(code as i32));
        }
        Ok(Self(handle))
    }

    fn raw(&self) -> HANDLE {
        self.0
    }
}

#[cfg(windows)]
impl Drop for EventHandle {
    fn drop(&mut self) {
        // SAFETY: self.0 is a HANDLE returned by CreateEventW and owned here.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

#[cfg(windows)]
fn duration_to_timeout_ms(timeout: Option<std::time::Duration>) -> u32 {
    match timeout {
        None => INFINITE,
        Some(d) => {
            if d.is_zero() {
                return 0;
            }
            let millis = d.as_millis();
            if millis == 0 {
                1
            } else {
                millis.min(u128::from(MAX_TIMEOUT_MS)) as u32
            }
        }
    }
}

#[cfg(windows)]
fn dword_len(len: usize) -> u32 {
    len.min(u32::MAX as usize) as u32
}

#[cfg(windows)]
fn wait_for_overlapped_result(
    handle: HANDLE,
    overlapped: &mut OVERLAPPED,
    event: HANDLE,
    timeout_ms: u32,
) -> std::io::Result<u32> {
    // SAFETY: pointers are valid and owned for call duration.
    let wait = unsafe { WaitForSingleObject(event, timeout_ms) };
    match wait {
        WAIT_OBJECT_0 => {
            let mut transferred = 0u32;
            // SAFETY: overlapped corresponds to the active I/O operation.
            let ok = unsafe {
                GetOverlappedResult(handle, overlapped as *mut OVERLAPPED, &mut transferred, 0)
            };
            if ok == 0 {
                // SAFETY: GetLastError has no preconditions.
                let code = unsafe { GetLastError() };
                return Err(std::io::Error::from_raw_os_error(code as i32));
            }
            Ok(transferred)
        }
        WAIT_TIMEOUT => {
            // SAFETY: handle and overlapped refer to this in-flight operation.
            unsafe {
                CancelIoEx(handle, overlapped as *mut OVERLAPPED);
            }
            let mut _discarded = 0u32;
            // SAFETY: wait for operation completion after cancellation.
            let _ = unsafe {
                GetOverlappedResult(handle, overlapped as *mut OVERLAPPED, &mut _discarded, 1)
            };

            // The operation exceeded caller timeout. We intentionally normalize to
            // TimedOut, regardless of races in cancellation completion (e.g. peer
            // closing concurrently can surface BrokenPipe from GetLastError).
            Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "named-pipe I/O timed out",
            ))
        }
        _ => {
            // SAFETY: GetLastError has no preconditions.
            let code = unsafe { GetLastError() };
            Err(std::io::Error::from_raw_os_error(code as i32))
        }
    }
}

#[cfg(windows)]
fn read_file_with_timeout(handle: HANDLE, buf: &mut [u8], timeout_ms: u32) -> std::io::Result<usize> {
    let event = EventHandle::create_manual_reset()?;
    // SAFETY: zero-initialized OVERLAPPED is valid; hEvent set to owned event.
    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    overlapped.hEvent = event.raw();

    let requested = dword_len(buf.len());
    // SAFETY: pointers are valid for the requested byte count.
    let ok: BOOL = unsafe {
        ReadFile(
            handle,
            buf.as_mut_ptr(),
            requested,
            std::ptr::null_mut(),
            &mut overlapped as *mut OVERLAPPED,
        )
    };

    if ok == 0 {
        // SAFETY: GetLastError has no preconditions.
        let code = unsafe { GetLastError() };
        if code != ERROR_IO_PENDING {
            return Err(std::io::Error::from_raw_os_error(code as i32));
        }
    }

    let transferred = wait_for_overlapped_result(handle, &mut overlapped, event.raw(), timeout_ms)?;
    Ok(transferred as usize)
}

#[cfg(windows)]
fn write_file_with_timeout(handle: HANDLE, buf: &[u8], timeout_ms: u32) -> std::io::Result<usize> {
    let event = EventHandle::create_manual_reset()?;
    // SAFETY: zero-initialized OVERLAPPED is valid; hEvent set to owned event.
    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    overlapped.hEvent = event.raw();

    let requested = dword_len(buf.len());
    // SAFETY: pointers are valid for requested byte count.
    let ok: BOOL = unsafe {
        WriteFile(
            handle,
            buf.as_ptr(),
            requested,
            std::ptr::null_mut(),
            &mut overlapped as *mut OVERLAPPED,
        )
    };

    if ok == 0 {
        // SAFETY: GetLastError has no preconditions.
        let code = unsafe { GetLastError() };
        if code != ERROR_IO_PENDING {
            return Err(std::io::Error::from_raw_os_error(code as i32));
        }
    }

    let transferred = wait_for_overlapped_result(handle, &mut overlapped, event.raw(), timeout_ms)?;
    Ok(transferred as usize)
}
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
    read_timeout_ms: AtomicU32,
    write_timeout_ms: AtomicU32,
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
                    FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OVERLAPPED,
                    std::ptr::null_mut(),
                )
            };

            if handle != INVALID_HANDLE_VALUE {
                debug!(pipe = %pipe_name, "connected to named pipe");
                // SAFETY: handle was returned by CreateFileW and is owned here.
                let file = unsafe { std::fs::File::from_raw_handle(handle as RawHandle) };
                return Ok(Self {
                    file,
                    read_timeout_ms: AtomicU32::new(INFINITE),
                    write_timeout_ms: AtomicU32::new(INFINITE),
                });
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
            read_timeout_ms: AtomicU32::new(self.read_timeout_ms.load(Ordering::Relaxed)),
            write_timeout_ms: AtomicU32::new(self.write_timeout_ms.load(Ordering::Relaxed)),
        })
    }

    pub(crate) fn set_read_timeout(&self, timeout: Option<std::time::Duration>) -> Result<()> {
        self.read_timeout_ms
            .store(duration_to_timeout_ms(timeout), Ordering::Relaxed);
        Ok(())
    }

    pub(crate) fn set_write_timeout(&self, timeout: Option<std::time::Duration>) -> Result<()> {
        self.write_timeout_ms
            .store(duration_to_timeout_ms(timeout), Ordering::Relaxed);
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
        let timeout_ms = self.read_timeout_ms.load(Ordering::Relaxed);
        if timeout_ms == INFINITE {
            return self.file.read(buf);
        }
        read_file_with_timeout(self.file.as_raw_handle() as HANDLE, buf, timeout_ms)
    }
}

#[cfg(windows)]
impl Write for NamedPipeStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let timeout_ms = self.write_timeout_ms.load(Ordering::Relaxed);
        if timeout_ms == INFINITE {
            return self.file.write(buf);
        }
        write_file_with_timeout(self.file.as_raw_handle() as HANDLE, buf, timeout_ms)
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
                PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED,
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
        let stream = NamedPipeStream {
            file,
            read_timeout_ms: AtomicU32::new(INFINITE),
            write_timeout_ms: AtomicU32::new(INFINITE),
        };
        Ok(IpcStream::from_named_pipe(stream))
    }

    pub fn path(&self) -> &Path {
        Path::new(&self.pipe_name)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use std::sync::mpsc;
    use std::sync::atomic::Ordering;
    use std::thread;
    use std::time::Duration;

    use super::*;

    fn make_pipe_name(tag: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(format!(
            r"\\.\pipe\ipcprims-transport-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be after epoch")
                .as_nanos()
        ))
    }

    #[test]
    fn read_times_out_when_server_stalls() {
        let pipe = make_pipe_name("read-timeout");
        let listener = NamedPipeListener::bind(&pipe).expect("listener should bind");
        let (tx, rx) = mpsc::channel::<()>();

        let server = thread::spawn(move || {
            let _stream = listener.accept().expect("listener should accept");
            // Keep the server side open long enough to force client-side timeout,
            // without writing bytes to satisfy the read.
            let _ = rx.recv_timeout(Duration::from_secs(2));
        });

        let mut stream = NamedPipeStream::connect_raw(&pipe).expect("client should connect");
        stream
            .set_read_timeout(Some(Duration::from_millis(50)))
            .expect("timeout setter should succeed");

        let mut one = [0u8; 1];
        let err = stream
            .read(&mut one)
            .expect_err("read should time out when server does not write");
        assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);

        let _ = tx.send(());
        server.join().expect("server thread should complete");
    }

    #[test]
    fn write_timeout_setter_and_clone_propagate() {
        let pipe = make_pipe_name("write-timeout");
        let listener = NamedPipeListener::bind(&pipe).expect("listener should bind");
        let (tx, rx) = mpsc::channel::<()>();

        let server = thread::spawn(move || {
            let _stream = listener.accept().expect("listener should accept");
            // Keep server side connected but intentionally not reading to create
            // backpressure and force client write timeout behavior.
            let _ = rx.recv_timeout(Duration::from_secs(3));
        });

        let stream = NamedPipeStream::connect_raw(&pipe).expect("client should connect");
        stream
            .set_write_timeout(Some(Duration::from_millis(50)))
            .expect("timeout setter should succeed");

        let cloned = stream.try_clone().expect("stream should clone");
        cloned
            .set_write_timeout(Some(Duration::from_millis(25)))
            .expect("timeout setter on clone should succeed");

        let mut writer = cloned;
        let chunk = vec![0x5Au8; 32 * 1024];
        let mut observed_timeout = false;

        // Keep writing until pipe buffers fill and overlapped write hits timeout.
        for _ in 0..512 {
            match writer.write(&chunk) {
                Ok(_) => continue,
                Err(err) => {
                    assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
                    observed_timeout = true;
                    break;
                }
            }
        }

        assert!(
            observed_timeout,
            "expected cloned stream write path to observe TimedOut under backpressure"
        );

        let _ = tx.send(());
        server.join().expect("server thread should complete");
    }

    #[test]
    fn timeout_setters_support_reset_to_infinite() {
        let pipe = make_pipe_name("timeout-reset");
        let listener = NamedPipeListener::bind(&pipe).expect("listener should bind");
        let (tx, rx) = mpsc::channel::<()>();

        let server = thread::spawn(move || {
            let _stream = listener.accept().expect("listener should accept");
            let _ = rx.recv_timeout(Duration::from_secs(2));
        });

        let stream = NamedPipeStream::connect_raw(&pipe).expect("client should connect");
        stream
            .set_read_timeout(Some(Duration::from_millis(25)))
            .expect("read timeout setter should succeed");
        stream
            .set_write_timeout(Some(Duration::from_millis(25)))
            .expect("write timeout setter should succeed");

        stream
            .set_read_timeout(None)
            .expect("read timeout reset should succeed");
        stream
            .set_write_timeout(None)
            .expect("write timeout reset should succeed");

        assert_eq!(stream.read_timeout_ms.load(Ordering::Relaxed), INFINITE);
        assert_eq!(stream.write_timeout_ms.load(Ordering::Relaxed), INFINITE);

        let _ = tx.send(());
        server.join().expect("server thread should complete");
    }

    #[test]
    fn duration_to_timeout_ms_clamps_large_values() {
        let huge = Duration::from_secs(u64::MAX / 2);
        assert_eq!(duration_to_timeout_ms(Some(huge)), MAX_TIMEOUT_MS);
        assert_eq!(duration_to_timeout_ms(None), INFINITE);
        assert_eq!(duration_to_timeout_ms(Some(Duration::from_nanos(1))), 1);
    }
}

