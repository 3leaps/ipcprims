use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

use tracing::{debug, info};

use crate::error::{Result, TransportError};
use crate::traits::IpcStream;

/// Unix domain socket transport.
///
/// Provides bind/accept/connect over filesystem-path UDS on Linux and macOS.
/// On Linux, abstract namespace sockets are preferred (no filesystem cleanup).
/// On macOS, filesystem paths are used with automatic cleanup via `Drop`.
pub struct UnixDomainSocket {
    listener: UnixListener,
    path: PathBuf,
    created_inode: Option<(u64, u64)>,
    /// Whether the path should be removed on drop (filesystem sockets only).
    cleanup_on_drop: bool,
}

impl UnixDomainSocket {
    /// Default permission mode for created socket paths.
    pub const DEFAULT_SOCKET_MODE: u32 = 0o600;
    /// Maximum socket path length.
    /// Unix `sockaddr_un.sun_path` is typically 108 bytes on Linux, 104 on macOS.
    #[cfg(target_os = "linux")]
    const MAX_PATH_LEN: usize = 108;
    #[cfg(target_os = "macos")]
    const MAX_PATH_LEN: usize = 104;
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    const MAX_PATH_LEN: usize = 104;

    /// Bind and listen on a filesystem-path Unix domain socket.
    ///
    /// The socket file is created at `path`. If the file already exists and is
    /// a socket, it is removed first (stale socket cleanup).
    pub fn bind(path: impl AsRef<Path>) -> Result<Self> {
        Self::bind_with_mode(path, Self::DEFAULT_SOCKET_MODE)
    }

    /// Bind and listen on a filesystem-path Unix domain socket with explicit mode.
    pub fn bind_with_mode(path: impl AsRef<Path>, mode: u32) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        // Validate path length
        let path_bytes = path.as_os_str().len();
        if path_bytes >= Self::MAX_PATH_LEN {
            return Err(TransportError::PathTooLong {
                path,
                len: path_bytes,
                max: Self::MAX_PATH_LEN,
            });
        }

        // Remove stale socket if it exists, but never remove non-socket files.
        if path.exists() {
            let metadata = std::fs::symlink_metadata(&path).map_err(|e| TransportError::Bind {
                path: path.clone(),
                source: e,
            })?;
            if metadata.file_type().is_socket() {
                debug!(?path, "removing stale socket");
                std::fs::remove_file(&path).map_err(|e| TransportError::Bind {
                    path: path.clone(),
                    source: e,
                })?;
            } else {
                return Err(TransportError::Bind {
                    path: path.clone(),
                    source: std::io::Error::new(
                        std::io::ErrorKind::AlreadyExists,
                        "existing path is not a unix socket",
                    ),
                });
            }
        }

        let listener = UnixListener::bind(&path).map_err(|e| TransportError::Bind {
            path: path.clone(),
            source: e,
        })?;

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).map_err(|e| {
            TransportError::Bind {
                path: path.clone(),
                source: e,
            }
        })?;
        let created_metadata =
            std::fs::symlink_metadata(&path).map_err(|e| TransportError::Bind {
                path: path.clone(),
                source: e,
            })?;
        let created_inode = Some((created_metadata.dev(), created_metadata.ino()));

        info!(?path, "listening on unix domain socket");

        Ok(Self {
            listener,
            path,
            created_inode,
            cleanup_on_drop: true,
        })
    }

    /// Accept an incoming connection (blocking).
    pub fn accept(&self) -> Result<IpcStream> {
        let (stream, _addr) = self.listener.accept().map_err(TransportError::Accept)?;
        debug!("accepted connection");
        Ok(IpcStream::from_unix(stream))
    }

    /// Connect to a listening Unix domain socket (blocking).
    pub fn connect(path: impl AsRef<Path>) -> Result<IpcStream> {
        let path = path.as_ref();
        let stream =
            std::os::unix::net::UnixStream::connect(path).map_err(|e| TransportError::Connect {
                path: path.to_path_buf(),
                source: e,
            })?;
        debug!(?path, "connected to unix domain socket");
        Ok(IpcStream::from_unix(stream))
    }

    /// The path this socket is bound to.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Transport name for diagnostics.
    pub fn transport_name(&self) -> &'static str {
        "unix-domain-socket"
    }
}

impl Drop for UnixDomainSocket {
    fn drop(&mut self) {
        if self.cleanup_on_drop {
            if let Some((expected_dev, expected_ino)) = self.created_inode {
                if let Ok(metadata) = std::fs::symlink_metadata(&self.path) {
                    if metadata.file_type().is_socket()
                        && metadata.dev() == expected_dev
                        && metadata.ino() == expected_ino
                    {
                        debug!(path = ?self.path, "cleaning up socket file");
                        let _ = std::fs::remove_file(&self.path);
                    } else {
                        debug!(
                            path = ?self.path,
                            "socket path identity changed; skipping cleanup"
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn test_bind_accept_connect() {
        let dir = std::env::temp_dir().join(format!("ipcprims-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock_path = dir.join("test.sock");

        let listener = UnixDomainSocket::bind(&sock_path).unwrap();
        assert!(sock_path.exists());

        // Connect from another thread
        let path_clone = sock_path.clone();
        let handle = std::thread::spawn(move || {
            let mut client = UnixDomainSocket::connect(&path_clone).unwrap();
            client.write_all(b"hello").unwrap();
        });

        let mut server = listener.accept().unwrap();
        let mut buf = [0u8; 5];
        server.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"hello");

        handle.join().unwrap();

        // Cleanup
        drop(listener);
        assert!(
            !sock_path.exists(),
            "socket file should be cleaned up on drop"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_path_too_long() {
        let long_path = "/tmp/".to_string() + &"a".repeat(200) + ".sock";
        let result = UnixDomainSocket::bind(&long_path);
        assert!(matches!(result, Err(TransportError::PathTooLong { .. })));
    }

    #[test]
    fn test_bind_default_permissions_hardened() {
        let dir = std::env::temp_dir().join(format!("ipcprims-perms-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock_path = dir.join("perm.sock");

        let listener = UnixDomainSocket::bind(&sock_path).unwrap();
        let mode = std::fs::metadata(&sock_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        drop(listener);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_bind_rejects_existing_non_socket_file() {
        let dir = std::env::temp_dir().join(format!("ipcprims-bind-file-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock_path = dir.join("not-a-socket.sock");
        std::fs::write(&sock_path, b"regular-file").unwrap();

        let result = UnixDomainSocket::bind(&sock_path);
        assert!(matches!(result, Err(TransportError::Bind { .. })));

        let _ = std::fs::remove_file(&sock_path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_drop_does_not_remove_replaced_path() {
        let dir = std::env::temp_dir().join(format!("ipcprims-drop-race-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock_path = dir.join("drop.sock");

        let listener = UnixDomainSocket::bind(&sock_path).unwrap();
        assert!(sock_path.exists());

        // Replace path while listener is alive.
        std::fs::remove_file(&sock_path).unwrap();
        std::fs::write(&sock_path, b"replacement-file").unwrap();

        drop(listener);
        assert!(
            sock_path.exists(),
            "drop must not remove path if inode identity changed"
        );

        let _ = std::fs::remove_file(&sock_path);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
