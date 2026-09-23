//! Platform-reported observations about a connected peer.
//!
//! Peer evidence is not authentication. Applications decide whether the
//! available observations are sufficient for their own authorization policy.

/// The platform facility that supplied Unix peer evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PeerEvidenceSource {
    /// Linux `SO_PEERCRED`.
    SoPeercred,
    /// macOS `getpeereid` supplied uid/gid.
    Getpeereid,
    /// macOS `LOCAL_PEERPID` supplied pid.
    LocalPeerpid,
}

/// Why peer evidence could not be observed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PeerEvidenceUnavailableReason {
    /// This platform does not yet have a peer-evidence implementation.
    UnsupportedPlatform,
    /// The platform did not provide peer credentials for this stream.
    QueryFailed,
    /// A pid lookup succeeded but returned an unusable pid or size.
    InvalidPid,
}

/// Unix peer credentials and the provenance of each observed value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct UnixPeerEvidence {
    pub uid: u32,
    pub gid: u32,
    pub pid: Option<u32>,
    /// Source of the uid and gid pair.
    pub uid_gid_source: PeerEvidenceSource,
    /// Source of the pid, when present.
    pub pid_source: Option<PeerEvidenceSource>,
    /// Explanation when no usable pid was observed.
    pub pid_unavailable_reason: Option<PeerEvidenceUnavailableReason>,
}

/// Evidence observed from the connected peer, or an explicit unavailable reason.
///
/// This is an observation, not an authentication or authorization decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PeerEvidence {
    /// Unix credentials. A pid can be unavailable, including Linux pid 0
    /// reported across pid namespaces.
    Unix(UnixPeerEvidence),
    /// No trustworthy platform evidence was available.
    Unavailable {
        reason: PeerEvidenceUnavailableReason,
    },
}

#[cfg(target_os = "linux")]
pub(crate) fn linux_peer_evidence((uid, gid, pid): (u32, u32, u32)) -> PeerEvidence {
    PeerEvidence::Unix(UnixPeerEvidence {
        uid,
        gid,
        pid: (pid != 0).then_some(pid),
        uid_gid_source: PeerEvidenceSource::SoPeercred,
        pid_source: (pid != 0).then_some(PeerEvidenceSource::SoPeercred),
        pid_unavailable_reason: (pid == 0).then_some(PeerEvidenceUnavailableReason::InvalidPid),
    })
}

#[cfg(target_os = "macos")]
pub(crate) fn legacy_macos_credentials(evidence: PeerEvidence) -> Option<(u32, u32, u32)> {
    match evidence {
        PeerEvidence::Unix(UnixPeerEvidence {
            uid,
            gid,
            pid: Some(pid),
            ..
        }) => Some((uid, gid, pid)),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn macos_peer_evidence(fd: std::os::fd::RawFd) -> PeerEvidence {
    let mut uid: libc::uid_t = 0;
    let mut gid: libc::gid_t = 0;

    // SAFETY: uid and gid are valid writable pointers; getpeereid only reads fd.
    if unsafe { libc::getpeereid(fd, &mut uid, &mut gid) } != 0 {
        return PeerEvidence::Unavailable {
            reason: PeerEvidenceUnavailableReason::QueryFailed,
        };
    }

    let mut pid: libc::pid_t = 0;
    let mut len = std::mem::size_of::<libc::pid_t>() as libc::socklen_t;
    // SAFETY: pid and len are writable and sized for a pid_t; getsockopt only reads fd.
    let rc = unsafe {
        libc::getsockopt(
            fd,
            libc::SOL_LOCAL,
            libc::LOCAL_PEERPID,
            (&mut pid as *mut libc::pid_t).cast::<libc::c_void>(),
            &mut len,
        )
    };
    macos_observation(uid, gid, rc == 0, len as usize, pid)
}

#[cfg(target_os = "macos")]
fn macos_observation(
    uid: u32,
    gid: u32,
    query_ok: bool,
    len: usize,
    pid: libc::pid_t,
) -> PeerEvidence {
    let reason = if !query_ok {
        Some(PeerEvidenceUnavailableReason::QueryFailed)
    } else if len != std::mem::size_of::<libc::pid_t>() || pid <= 0 {
        Some(PeerEvidenceUnavailableReason::InvalidPid)
    } else {
        None
    };
    PeerEvidence::Unix(UnixPeerEvidence {
        uid,
        gid,
        pid: reason.is_none().then_some(pid as u32),
        uid_gid_source: PeerEvidenceSource::Getpeereid,
        pid_source: reason.is_none().then_some(PeerEvidenceSource::LocalPeerpid),
        pid_unavailable_reason: reason,
    })
}

#[cfg(all(test, target_os = "macos"))]
pub(crate) mod tests {
    use super::*;
    use crate::IpcStream;
    use std::io::{Read, Write};
    use std::process::{Child, Command, Stdio};

    pub(crate) fn spawn_client(path: &std::path::Path) -> Child {
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "peer_evidence::tests::macos_child_client",
                "--nocapture",
            ])
            .env("IPCPRIMS_TEST_PEER_SOCKET", path)
            .stdout(Stdio::piped())
            .spawn()
            .unwrap()
    }

    pub(crate) fn client_ids(child: Child) -> (u32, u32) {
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let text = String::from_utf8(output.stdout).unwrap();
        let fields = text
            .split("PEER_IDS=")
            .nth(1)
            .expect("client effective IDs");
        let mut fields = fields.split_whitespace().next().unwrap().split(',');
        (
            fields.next().unwrap().parse().unwrap(),
            fields.next().unwrap().parse().unwrap(),
        )
    }

    #[test]
    fn macos_child_client() {
        let Ok(path) = std::env::var("IPCPRIMS_TEST_PEER_SOCKET") else {
            return;
        };
        let mut stream = std::os::unix::net::UnixStream::connect(path).unwrap();
        println!("PEER_IDS={},{}", unsafe { libc::geteuid() }, unsafe {
            libc::getegid()
        });
        let mut ack = [0];
        stream.read_exact(&mut ack).unwrap();
    }

    #[test]
    fn macos_connected_stream_reports_current_process() {
        let dir = std::env::temp_dir().join(format!("ip-s-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("peer.sock");
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        let child = spawn_client(&path);
        let (mut stream, _) = listener.accept().unwrap();
        let pid = child.id();
        let server = IpcStream::from_unix(stream.try_clone().unwrap());
        let credentials = server.peer_credentials();
        let evidence = server.peer_evidence();
        stream.write_all(&[1]).unwrap();
        let (uid, gid) = client_ids(child);
        let expected = (uid, gid, pid);

        assert_eq!(credentials, Some(expected));
        assert_eq!(
            evidence,
            PeerEvidence::Unix(UnixPeerEvidence {
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

    #[test]
    fn invalid_fd_has_explicit_unavailable_reason() {
        assert_eq!(
            macos_peer_evidence(-1),
            PeerEvidence::Unavailable {
                reason: PeerEvidenceUnavailableReason::QueryFailed,
            }
        );
    }

    #[test]
    fn missing_pid_does_not_become_a_legacy_tuple() {
        assert_eq!(
            legacy_macos_credentials(PeerEvidence::Unix(UnixPeerEvidence {
                uid: 501,
                gid: 20,
                pid: None,
                uid_gid_source: PeerEvidenceSource::Getpeereid,
                pid_source: None,
                pid_unavailable_reason: Some(PeerEvidenceUnavailableReason::QueryFailed),
            })),
            None
        );
        assert_eq!(
            macos_observation(501, 20, false, 4, 0),
            PeerEvidence::Unix(UnixPeerEvidence {
                uid: 501,
                gid: 20,
                pid: None,
                uid_gid_source: PeerEvidenceSource::Getpeereid,
                pid_source: None,
                pid_unavailable_reason: Some(PeerEvidenceUnavailableReason::QueryFailed),
            })
        );
        assert_eq!(
            legacy_macos_credentials(macos_observation(501, 20, true, 4, 0)),
            None
        );
        assert_eq!(
            legacy_macos_credentials(macos_observation(501, 20, true, 0, 123)),
            None
        );
    }
}

#[cfg(all(test, target_os = "linux"))]
mod linux_tests {
    use super::*;

    #[test]
    fn pid_zero_is_absent_only_in_typed_evidence() {
        assert_eq!(
            linux_peer_evidence((1000, 1000, 0)),
            PeerEvidence::Unix(UnixPeerEvidence {
                uid: 1000,
                gid: 1000,
                pid: None,
                uid_gid_source: PeerEvidenceSource::SoPeercred,
                pid_source: None,
                pid_unavailable_reason: Some(PeerEvidenceUnavailableReason::InvalidPid),
            })
        );
    }
}
