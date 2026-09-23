//! Tokio async Unix domain socket transport (Unix only).
//!
//! These types are intentionally gated behind `#[cfg(all(unix, feature = "async"))]`.

use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::UnixListener;
use tracing::{debug, info};

use crate::error::{Result, TransportError};
use crate::peer_evidence::PeerEvidence;
#[cfg(not(target_os = "macos"))]
use crate::peer_evidence::PeerEvidenceUnavailableReason;

/// An async IPC stream (Tokio).
///
/// On Unix this is a Unix domain socket stream.
pub struct AsyncIpcStream {
    inner: tokio::net::UnixStream,
}

impl AsyncIpcStream {
    /// Wrap an existing Tokio UnixStream.
    pub fn new(inner: tokio::net::UnixStream) -> Self {
        Self { inner }
    }

    /// Convert from a blocking std UnixStream.
    pub fn from_std(stream: std::os::unix::net::UnixStream) -> std::io::Result<Self> {
        tokio::net::UnixStream::from_std(stream).map(Self::new)
    }

    /// Split into owned read/write halves for concurrent driving.
    pub fn into_split(self) -> (tokio::io::ReadHalf<Self>, tokio::io::WriteHalf<Self>) {
        tokio::io::split(self)
    }

    /// Access the underlying Tokio stream by reference.
    pub fn get_ref(&self) -> &tokio::net::UnixStream {
        &self.inner
    }

    /// Access the underlying Tokio stream mutably.
    pub fn get_mut(&mut self) -> &mut tokio::net::UnixStream {
        &mut self.inner
    }

    /// Consume and return the underlying Tokio stream.
    pub fn into_inner(self) -> tokio::net::UnixStream {
        self.inner
    }

    /// Get the credentials of the connected peer (Linux or macOS).
    ///
    /// Returns `(uid, gid, pid)` via `SO_PEERCRED`, or `None` if unavailable.
    /// Linux's historical tuple retains pid 0 across pid namespaces; use
    /// [`Self::peer_evidence`] for an optional pid.
    #[cfg(target_os = "linux")]
    pub fn peer_credentials(&self) -> Option<(u32, u32, u32)> {
        use std::os::fd::AsRawFd;

        let fd = self.inner.as_raw_fd();
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

    /// Get the credentials of the connected peer (macOS).
    ///
    /// Returns `None` unless `getpeereid` and `LOCAL_PEERPID` both succeed
    /// and report a usable pid. Peer credentials are not authentication.
    #[cfg(target_os = "macos")]
    pub fn peer_credentials(&self) -> Option<(u32, u32, u32)> {
        crate::peer_evidence::legacy_macos_credentials(self.peer_evidence())
    }

    /// Get the credentials of the connected peer.
    ///
    /// Returns `None` on platforms that do not expose peer credentials.
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    pub fn peer_credentials(&self) -> Option<(u32, u32, u32)> {
        None
    }

    /// Observe platform peer evidence without treating it as authentication.
    pub fn peer_evidence(&self) -> PeerEvidence {
        #[cfg(target_os = "linux")]
        {
            return match self.peer_credentials() {
                Some(credentials) => crate::peer_evidence::linux_peer_evidence(credentials),
                None => PeerEvidence::Unavailable {
                    reason: PeerEvidenceUnavailableReason::QueryFailed,
                },
            };
        }
        #[cfg(target_os = "macos")]
        {
            use std::os::fd::AsRawFd;
            crate::peer_evidence::macos_peer_evidence(self.inner.as_raw_fd())
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            PeerEvidence::Unavailable {
                reason: PeerEvidenceUnavailableReason::UnsupportedPlatform,
            }
        }
    }
}

impl std::fmt::Debug for AsyncIpcStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncIpcStream")
            .field("type", &"unix")
            .finish()
    }
}

impl AsyncRead for AsyncIpcStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for AsyncIpcStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.get_mut().inner).poll_write(cx, data)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

/// Async Unix domain socket listener.
pub struct AsyncUnixDomainSocket {
    listener: UnixListener,
    path: PathBuf,
    created_inode: Option<(u64, u64)>,
    cleanup_on_drop: bool,
}

impl AsyncUnixDomainSocket {
    /// Default permission mode for created socket paths.
    pub const DEFAULT_SOCKET_MODE: u32 = 0o600;
    /// Maximum socket path length.
    #[cfg(target_os = "linux")]
    const MAX_PATH_LEN: usize = 108;
    #[cfg(target_os = "macos")]
    const MAX_PATH_LEN: usize = 104;
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    const MAX_PATH_LEN: usize = 104;

    /// Bind and listen on a filesystem-path Unix domain socket.
    pub fn bind(path: impl AsRef<Path>) -> Result<Self> {
        Self::bind_with_mode(path, Self::DEFAULT_SOCKET_MODE)
    }

    /// Bind and listen on a filesystem-path Unix domain socket with explicit mode.
    pub fn bind_with_mode(path: impl AsRef<Path>, mode: u32) -> Result<Self> {
        let path = path.as_ref().to_path_buf();

        let path_bytes = path.as_os_str().len();
        if path_bytes >= Self::MAX_PATH_LEN {
            return Err(TransportError::PathTooLong {
                path,
                len: path_bytes,
                max: Self::MAX_PATH_LEN,
            });
        }

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

        let std_listener =
            std::os::unix::net::UnixListener::bind(&path).map_err(|e| TransportError::Bind {
                path: path.clone(),
                source: e,
            })?;
        std_listener
            .set_nonblocking(true)
            .map_err(|e| TransportError::Bind {
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

        let listener = UnixListener::from_std(std_listener).map_err(TransportError::Io)?;
        info!(?path, "listening on unix domain socket (async)");

        Ok(Self {
            listener,
            path,
            created_inode,
            cleanup_on_drop: true,
        })
    }

    /// Accept an incoming connection (async).
    pub async fn accept(&self) -> Result<AsyncIpcStream> {
        let (stream, _addr) = self
            .listener
            .accept()
            .await
            .map_err(TransportError::Accept)?;
        debug!("accepted connection");
        Ok(AsyncIpcStream::new(stream))
    }

    /// Connect to a listening Unix domain socket (async).
    pub async fn connect(path: impl AsRef<Path>) -> Result<AsyncIpcStream> {
        let path = path.as_ref().to_path_buf();
        let stream =
            tokio::net::UnixStream::connect(&path)
                .await
                .map_err(|e| TransportError::Connect {
                    path: path.clone(),
                    source: e,
                })?;
        debug!(?path, "connected to unix domain socket (async)");
        Ok(AsyncIpcStream::new(stream))
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

impl Drop for AsyncUnixDomainSocket {
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

#[cfg(all(test, unix, feature = "async"))]
mod tests {
    use std::os::unix::fs::{FileTypeExt, PermissionsExt};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    #[tokio::test]
    #[cfg(target_os = "linux")]
    async fn peer_credentials_linux_match_typed_evidence() {
        let (client, server) = tokio::net::UnixStream::pair().unwrap();
        let server = AsyncIpcStream::new(server);
        let (uid, gid, pid) = server.peer_credentials().expect("Linux SO_PEERCRED");
        let PeerEvidence::Unix(evidence) = server.peer_evidence() else {
            panic!("expected Unix evidence");
        };
        assert_eq!(
            (evidence.uid, evidence.gid, evidence.pid),
            (uid, gid, (pid != 0).then_some(pid))
        );
        assert_eq!(
            evidence.uid_gid_source,
            crate::PeerEvidenceSource::SoPeercred
        );
        assert_eq!(
            evidence.pid_source,
            (pid != 0).then_some(crate::PeerEvidenceSource::SoPeercred)
        );
        drop(client);
    }
    #[cfg(target_os = "macos")]
    use crate::peer_evidence::PeerEvidenceSource;

    #[tokio::test]
    #[cfg(target_os = "macos")]
    async fn peer_credentials_macos() {
        use tokio::io::AsyncWriteExt;
        let dir = std::env::temp_dir().join(format!("ip-a-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("peer.sock");
        let listener = tokio::net::UnixListener::bind(&path).unwrap();
        let child = crate::peer_evidence::tests::spawn_client(&path);
        let (stream, _) = listener.accept().await.unwrap();
        let pid = child.id();
        let server = AsyncIpcStream::new(stream);
        let credentials = server.peer_credentials();
        let evidence = server.peer_evidence();
        let mut stream = server.into_inner();
        stream.write_all(&[1]).await.unwrap();
        let (uid, gid) =
            tokio::task::spawn_blocking(move || crate::peer_evidence::tests::client_ids(child))
                .await
                .unwrap();
        let expected = (uid, gid, pid);

        assert_eq!(credentials, Some(expected));
        assert_eq!(
            evidence,
            PeerEvidence::Unix(crate::UnixPeerEvidence {
                uid: expected.0,
                gid: expected.1,
                pid: Some(expected.2),
                uid_gid_source: PeerEvidenceSource::Getpeereid,
                pid_source: Some(PeerEvidenceSource::LocalPeerpid),
                pid_unavailable_reason: None,
            })
        );
        drop(listener);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }

    #[tokio::test]
    async fn bind_accept_connect() {
        let dir = std::env::temp_dir().join(format!("ipcprims-async-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock_path = dir.join("test.sock");

        let listener = AsyncUnixDomainSocket::bind(&sock_path).unwrap();
        assert!(sock_path.exists());

        let server = tokio::spawn(async move {
            let mut server = listener.accept().await.unwrap();
            let mut buf = [0u8; 5];
            server.read_exact(&mut buf).await.unwrap();
            assert_eq!(&buf, b"hello");
        });

        let mut client = AsyncUnixDomainSocket::connect(&sock_path).await.unwrap();
        client.write_all(b"hello").await.unwrap();
        server.await.unwrap();

        drop(client);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn path_too_long() {
        let long_path = "/tmp/".to_string() + &"a".repeat(200) + ".sock";
        let result = AsyncUnixDomainSocket::bind(&long_path);
        assert!(matches!(result, Err(TransportError::PathTooLong { .. })));
    }

    #[tokio::test]
    async fn bind_default_permissions_hardened() {
        let dir = std::env::temp_dir().join(format!("ipcprims-async-perms-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock_path = dir.join("perm.sock");

        let listener = AsyncUnixDomainSocket::bind(&sock_path).unwrap();
        let mode = std::fs::metadata(&sock_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);

        drop(listener);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn bind_rejects_existing_non_socket_file() {
        let dir =
            std::env::temp_dir().join(format!("ipcprims-async-bind-file-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock_path = dir.join("not-a-socket.sock");
        std::fs::write(&sock_path, b"regular-file").unwrap();

        let result = AsyncUnixDomainSocket::bind(&sock_path);
        assert!(matches!(result, Err(TransportError::Bind { .. })));

        let _ = std::fs::remove_file(&sock_path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn bind_rejects_existing_directory() {
        let dir =
            std::env::temp_dir().join(format!("ipcprims-async-bind-dir-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock_path = dir.join("dir-at-socket-path.sock");
        std::fs::create_dir_all(&sock_path).unwrap();

        let result = AsyncUnixDomainSocket::bind(&sock_path);
        assert!(matches!(result, Err(TransportError::Bind { .. })));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn drop_does_not_remove_replaced_path() {
        let dir =
            std::env::temp_dir().join(format!("ipcprims-async-drop-race-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock_path = dir.join("drop.sock");

        let listener = AsyncUnixDomainSocket::bind(&sock_path).unwrap();
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

    #[tokio::test]
    async fn drop_cleans_socket_file_when_identity_matches() {
        let dir =
            std::env::temp_dir().join(format!("ipcprims-async-drop-clean-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock_path = dir.join("clean.sock");

        let listener = AsyncUnixDomainSocket::bind(&sock_path).unwrap();
        let created = std::fs::symlink_metadata(&sock_path).unwrap();
        assert!(created.file_type().is_socket());

        drop(listener);
        assert!(
            !sock_path.exists(),
            "socket file should be cleaned up on drop"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
