//! Tokio async Windows named-pipe transport.
//!
//! These types are intentionally gated behind `#[cfg(all(windows, feature = "async"))]`.

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};
use tracing::debug;

use crate::error::{Result, TransportError};

fn normalize_pipe_name(path: &Path) -> String {
    let raw = path.as_os_str().to_string_lossy().to_string();
    if raw.starts_with(r"\\.\pipe\") {
        raw
    } else {
        format!(r"\\.\pipe\{raw}")
    }
}

/// An async IPC stream (Tokio).
///
/// On Windows this is a named-pipe client/server handle.
pub struct AsyncIpcStream {
    inner: AsyncIpcStreamInner,
}

enum AsyncIpcStreamInner {
    Client(NamedPipeClient),
    Server(NamedPipeServer),
}

impl AsyncIpcStream {
    /// Wrap an existing Tokio named-pipe client handle.
    pub fn new(inner: NamedPipeClient) -> Self {
        Self {
            inner: AsyncIpcStreamInner::Client(inner),
        }
    }

    /// Wrap an existing Tokio named-pipe server handle.
    pub fn from_server(inner: NamedPipeServer) -> Self {
        Self {
            inner: AsyncIpcStreamInner::Server(inner),
        }
    }

    /// Split into read/write halves for concurrent driving.
    pub fn into_split(self) -> (tokio::io::ReadHalf<Self>, tokio::io::WriteHalf<Self>) {
        tokio::io::split(self)
    }
}

impl std::fmt::Debug for AsyncIpcStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncIpcStream")
            .field("type", &"named-pipe")
            .finish()
    }
}

impl AsyncRead for AsyncIpcStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match &mut self.get_mut().inner {
            AsyncIpcStreamInner::Client(inner) => Pin::new(inner).poll_read(cx, buf),
            AsyncIpcStreamInner::Server(inner) => Pin::new(inner).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for AsyncIpcStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match &mut self.get_mut().inner {
            AsyncIpcStreamInner::Client(inner) => Pin::new(inner).poll_write(cx, data),
            AsyncIpcStreamInner::Server(inner) => Pin::new(inner).poll_write(cx, data),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match &mut self.get_mut().inner {
            AsyncIpcStreamInner::Client(inner) => Pin::new(inner).poll_flush(cx),
            AsyncIpcStreamInner::Server(inner) => Pin::new(inner).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match &mut self.get_mut().inner {
            AsyncIpcStreamInner::Client(inner) => Pin::new(inner).poll_shutdown(cx),
            AsyncIpcStreamInner::Server(inner) => Pin::new(inner).poll_shutdown(cx),
        }
    }
}

/// Async Windows named-pipe listener facade.
pub struct AsyncNamedPipeSocket {
    path: PathBuf,
}

impl AsyncNamedPipeSocket {
    /// Bind to a Windows named-pipe path.
    pub fn bind(path: impl AsRef<Path>) -> Result<Self> {
        let name = normalize_pipe_name(path.as_ref());
        Ok(Self {
            path: PathBuf::from(name),
        })
    }

    /// Accept one incoming connection (async).
    pub async fn accept(&self) -> Result<AsyncIpcStream> {
        let name = self.path.to_string_lossy().to_string();
        let server = ServerOptions::new()
            .create(&name)
            .map_err(TransportError::Io)?;

        connect_server(&server).await?;
        debug!(pipe = %name, "accepted connection on named pipe (async)");
        Ok(AsyncIpcStream::from_server(server))
    }

    /// Connect to a listening named pipe (async).
    pub async fn connect(path: impl AsRef<Path>) -> Result<AsyncIpcStream> {
        let name = normalize_pipe_name(path.as_ref());
        let mut retries = 0u32;
        loop {
            match ClientOptions::new().open(&name) {
                Ok(client) => {
                    debug!(pipe = %name, "connected to named pipe (async)");
                    return Ok(AsyncIpcStream::new(client));
                }
                Err(e) if retries < 200 => {
                    retries += 1;
                    if e.kind() == std::io::ErrorKind::NotFound
                        || e.raw_os_error()
                            == Some(windows_sys::Win32::Foundation::ERROR_PIPE_BUSY as i32)
                    {
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                        continue;
                    }
                    return Err(TransportError::Connect {
                        path: PathBuf::from(name),
                        source: e,
                    });
                }
                Err(e) => {
                    return Err(TransportError::Connect {
                        path: PathBuf::from(name),
                        source: e,
                    });
                }
            }
        }
    }

    /// Bound pipe path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Transport name for diagnostics.
    pub fn transport_name(&self) -> &'static str {
        "named-pipe"
    }
}

async fn connect_server(server: &NamedPipeServer) -> Result<()> {
    match server.connect().await {
        Ok(()) => Ok(()),
        Err(err)
            if err.kind() == std::io::ErrorKind::WouldBlock
                || err.raw_os_error()
                    == Some(windows_sys::Win32::Foundation::ERROR_PIPE_CONNECTED as i32) =>
        {
            // Client connected between instance creation and connect() call.
            Ok(())
        }
        Err(err) => Err(TransportError::Accept(err)),
    }?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    static COUNTER: AtomicU64 = AtomicU64::new(1);

    fn test_pipe_name(tag: &str) -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        PathBuf::from(format!(
            r"\\.\pipe\ipcprims-async-transport-{tag}-{}-{}",
            std::process::id(),
            n
        ))
    }

    /// Basic async transport roundtrip: server accepts, client writes, server
    /// reads the same bytes back.
    #[tokio::test]
    async fn async_roundtrip_on_named_pipe() {
        let pipe = test_pipe_name("roundtrip");
        let listener = AsyncNamedPipeSocket::bind(&pipe).expect("bind should succeed");

        let pipe_client = pipe.clone();
        let client_task = tokio::spawn(async move {
            let mut stream = AsyncNamedPipeSocket::connect(&pipe_client)
                .await
                .expect("client connect");
            stream
                .write_all(b"hello-async")
                .await
                .expect("client write");
            stream.flush().await.expect("client flush");
            stream.shutdown().await.expect("client shutdown");
        });

        let mut server_stream = listener.accept().await.expect("accept");
        let mut buf = vec![0u8; 64];
        let n = server_stream.read(&mut buf).await.expect("server read");
        assert_eq!(&buf[..n], b"hello-async");

        client_task.await.expect("client task");
    }

    /// Async multi-client: accept two sequential clients on the same pipe name.
    /// Exercises pipe instance recreation in the async path.
    #[tokio::test]
    async fn async_multi_client_on_named_pipe() {
        let pipe = test_pipe_name("multi-async");

        // Client 1
        let listener1 = AsyncNamedPipeSocket::bind(&pipe).expect("bind should succeed");
        let pipe_c1 = pipe.clone();
        let c1 = tokio::spawn(async move {
            let mut stream = AsyncNamedPipeSocket::connect(&pipe_c1)
                .await
                .expect("client 1 connect");
            stream.write_all(b"c1").await.expect("c1 write");
            stream.flush().await.expect("c1 flush");
        });
        let mut s1 = listener1.accept().await.expect("accept 1");
        let mut buf = vec![0u8; 16];
        let n = s1.read(&mut buf).await.expect("read 1");
        assert_eq!(&buf[..n], b"c1");
        c1.await.expect("client 1 task");
        drop(s1);

        // Client 2 — new listener instance on the same pipe name
        let listener2 = AsyncNamedPipeSocket::bind(&pipe).expect("rebind should succeed");
        let pipe_c2 = pipe.clone();
        let c2 = tokio::spawn(async move {
            let mut stream = AsyncNamedPipeSocket::connect(&pipe_c2)
                .await
                .expect("client 2 connect");
            stream.write_all(b"c2").await.expect("c2 write");
            stream.flush().await.expect("c2 flush");
        });
        let mut s2 = listener2.accept().await.expect("accept 2");
        let mut buf2 = vec![0u8; 16];
        let n2 = s2.read(&mut buf2).await.expect("read 2");
        assert_eq!(&buf2[..n2], b"c2");
        c2.await.expect("client 2 task");
    }
}
