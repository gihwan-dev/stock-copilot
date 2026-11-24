use std::sync::Arc;
use std::time::Duration;

use application::{ExecutablePath, PeerProcessInspector, PeerProcessLookupError, ProcessId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UserId(u32);

impl UserId {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObservedPeerCredentials {
    uid: UserId,
    pid: Option<i32>,
}

impl ObservedPeerCredentials {
    #[must_use]
    pub const fn new(uid: UserId, pid: Option<i32>) -> Self {
        Self { uid, pid }
    }

    #[must_use]
    pub const fn uid(self) -> UserId {
        self.uid
    }

    #[must_use]
    pub const fn pid(self) -> Option<i32> {
        self.pid
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedPeerPolicy {
    uid: UserId,
    parent_pid: ProcessId,
    executable_path: ExecutablePath,
}

impl ExpectedPeerPolicy {
    #[must_use]
    pub const fn new(uid: UserId, parent_pid: ProcessId, executable_path: ExecutablePath) -> Self {
        Self {
            uid,
            parent_pid,
            executable_path,
        }
    }

    #[must_use]
    pub const fn expected_uid(&self) -> UserId {
        self.uid
    }

    #[must_use]
    pub const fn expected_parent_pid(&self) -> ProcessId {
        self.parent_pid
    }

    #[must_use]
    pub const fn expected_executable_path(&self) -> &ExecutablePath {
        &self.executable_path
    }
}

#[derive(Debug)]
pub enum PeerInspectorMode<P> {
    Configured(Arc<P>),
    Unconfigured,
}

impl<P> Clone for PeerInspectorMode<P> {
    fn clone(&self) -> Self {
        match self {
            Self::Configured(inspector) => Self::Configured(Arc::clone(inspector)),
            Self::Unconfigured => Self::Unconfigured,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerVerificationFailure {
    CredentialsUnavailable,
    UidMismatch,
    PidUnavailable,
    PidInvalid,
    InspectorMissing,
    InspectionTimedOut,
    InspectionFailed(PeerProcessLookupError),
    ParentPidMismatch,
    ExecutablePathMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPeer {
    uid: UserId,
    pid: ProcessId,
    parent_pid: ProcessId,
    executable_path: ExecutablePath,
}

impl VerifiedPeer {
    #[must_use]
    pub const fn uid(&self) -> UserId {
        self.uid
    }

    #[must_use]
    pub const fn pid(&self) -> ProcessId {
        self.pid
    }

    #[must_use]
    pub const fn parent_pid(&self) -> ProcessId {
        self.parent_pid
    }

    #[must_use]
    pub const fn executable_path(&self) -> &ExecutablePath {
        &self.executable_path
    }
}

pub async fn verify_peer<P: PeerProcessInspector>(
    observed: ObservedPeerCredentials,
    expected: &ExpectedPeerPolicy,
    inspector: &PeerInspectorMode<P>,
    timeout: Duration,
) -> Result<VerifiedPeer, PeerVerificationFailure> {
    if observed.uid() != expected.expected_uid() {
        return Err(PeerVerificationFailure::UidMismatch);
    }
    let Some(raw_pid) = observed.pid() else {
        return Err(PeerVerificationFailure::PidUnavailable);
    };
    let positive_pid =
        u32::try_from(raw_pid).map_err(|_error| PeerVerificationFailure::PidInvalid)?;
    let peer_pid =
        ProcessId::try_new(positive_pid).map_err(|_error| PeerVerificationFailure::PidInvalid)?;
    let PeerInspectorMode::Configured(port) = inspector else {
        return Err(PeerVerificationFailure::InspectorMissing);
    };
    let inspected = tokio::time::timeout(timeout, port.inspect(peer_pid))
        .await
        .map_err(|_elapsed| PeerVerificationFailure::InspectionTimedOut)?
        .map_err(PeerVerificationFailure::InspectionFailed)?;
    if inspected.parent_pid() != expected.expected_parent_pid() {
        return Err(PeerVerificationFailure::ParentPidMismatch);
    }
    if inspected.executable_path() != expected.expected_executable_path() {
        return Err(PeerVerificationFailure::ExecutablePathMismatch);
    }
    Ok(VerifiedPeer {
        uid: observed.uid(),
        pid: peer_pid,
        parent_pid: inspected.parent_pid(),
        executable_path: inspected.executable_path().clone(),
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use application::{
        ExecutablePath, PeerProcessInfo, PeerProcessInspector, PeerProcessLookupError, ProcessId,
    };
    use testkit_ports::system_fakes::ScriptedPeerProcessInspector;

    use super::{
        ExpectedPeerPolicy, ObservedPeerCredentials, PeerInspectorMode, PeerVerificationFailure,
        UserId, verify_peer,
    };

    struct SlowInspector;

    impl PeerProcessInspector for SlowInspector {
        async fn inspect(&self, _: ProcessId) -> Result<PeerProcessInfo, PeerProcessLookupError> {
            tokio::time::sleep(Duration::from_secs(10_u64)).await;
            Err(PeerProcessLookupError::Unavailable)
        }
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("테스트 런타임")
    }

    fn policy() -> ExpectedPeerPolicy {
        ExpectedPeerPolicy::new(
            UserId::new(501_u32),
            ProcessId::try_new(1000_u32).expect("부모 PID"),
            ExecutablePath::try_new("/Applications/StockCopilot.app/mcp-bridge").expect("경로"),
        )
    }

    async fn assert_unconfigured_rejection(
        observed: ObservedPeerCredentials,
        expected_err: PeerVerificationFailure,
        message: &str,
    ) {
        let expected = policy();
        let unconfigured: PeerInspectorMode<ScriptedPeerProcessInspector> =
            PeerInspectorMode::Unconfigured;
        assert_eq!(
            verify_peer(
                observed,
                &expected,
                &unconfigured,
                Duration::from_secs(1_u64)
            )
            .await,
            Err(expected_err),
            "{message}"
        );
    }

    #[test]
    fn peer_verification_rejects_bad_uid_pid_and_missing_inspector() {
        runtime().block_on(async {
            assert_unconfigured_rejection(
                ObservedPeerCredentials::new(UserId::new(502_u32), Some(2000_i32)),
                PeerVerificationFailure::UidMismatch,
                "UID가 다르면 거부한다",
            )
            .await;
            assert_unconfigured_rejection(
                ObservedPeerCredentials::new(UserId::new(501_u32), None),
                PeerVerificationFailure::PidUnavailable,
                "PID가 없으면 거부한다",
            )
            .await;
            assert_unconfigured_rejection(
                ObservedPeerCredentials::new(UserId::new(501_u32), Some(0_i32)),
                PeerVerificationFailure::PidInvalid,
                "PID 0은 거부한다",
            )
            .await;
            assert_unconfigured_rejection(
                ObservedPeerCredentials::new(UserId::new(501_u32), Some(-5_i32)),
                PeerVerificationFailure::PidInvalid,
                "음수 PID는 거부한다",
            )
            .await;
            assert_unconfigured_rejection(
                ObservedPeerCredentials::new(UserId::new(501_u32), Some(2000_i32)),
                PeerVerificationFailure::InspectorMissing,
                "포트 미제공은 닫힌 실패로 거부한다",
            )
            .await;
        });
    }

    #[test]
    fn peer_verification_rejects_lookup_errors_and_mismatches() {
        runtime().block_on(async {
            let expected = policy();
            let timeout = Duration::from_secs(1_u64);
            let observed = ObservedPeerCredentials::new(UserId::new(501_u32), Some(2000_i32));
            let failing = Arc::new(ScriptedPeerProcessInspector::new([
                Err(PeerProcessLookupError::Unavailable),
                Err(PeerProcessLookupError::NotFound),
                Err(PeerProcessLookupError::PermissionDenied),
                Ok(PeerProcessInfo::new(
                    ProcessId::try_new(9999_u32).expect("다른 부모"),
                    expected.expected_executable_path().clone(),
                )),
                Ok(PeerProcessInfo::new(
                    expected.expected_parent_pid(),
                    ExecutablePath::try_new("/tmp/fake-bridge").expect("다른 경로"),
                )),
            ]));
            let mode = PeerInspectorMode::Configured(failing);
            let cases = [
                (
                    PeerVerificationFailure::InspectionFailed(PeerProcessLookupError::Unavailable),
                    "Unavailable 거부",
                ),
                (
                    PeerVerificationFailure::InspectionFailed(PeerProcessLookupError::NotFound),
                    "NotFound 거부",
                ),
                (
                    PeerVerificationFailure::InspectionFailed(
                        PeerProcessLookupError::PermissionDenied,
                    ),
                    "PermissionDenied 거부",
                ),
                (
                    PeerVerificationFailure::ParentPidMismatch,
                    "부모 PID 불일치",
                ),
                (
                    PeerVerificationFailure::ExecutablePathMismatch,
                    "실행 파일 경로 불일치",
                ),
            ];
            for (expected_err, msg) in cases {
                assert_eq!(
                    verify_peer(observed, &expected, &mode, timeout).await,
                    Err(expected_err),
                    "{msg}"
                );
            }
        });
    }

    #[test]
    fn peer_verification_rejects_lookup_timeout() {
        runtime().block_on(async {
            let expected = policy();
            let observed = ObservedPeerCredentials::new(UserId::new(501_u32), Some(2000_i32));
            assert_eq!(
                verify_peer(
                    observed,
                    &expected,
                    &PeerInspectorMode::Configured(Arc::new(SlowInspector)),
                    Duration::from_millis(30_u64),
                )
                .await,
                Err(PeerVerificationFailure::InspectionTimedOut),
                "조회 제한 시간 초과 거부"
            );
        });
    }

    #[test]
    fn peer_verification_accepts_matching_credentials_and_records_pid() {
        runtime().block_on(async {
            let expected = policy();
            let matching = Arc::new(ScriptedPeerProcessInspector::new([Ok(
                PeerProcessInfo::new(
                    expected.expected_parent_pid(),
                    expected.expected_executable_path().clone(),
                ),
            )]));
            let verified = verify_peer(
                ObservedPeerCredentials::new(UserId::new(501_u32), Some(2000_i32)),
                &expected,
                &PeerInspectorMode::Configured(Arc::clone(&matching)),
                Duration::from_secs(1_u64),
            )
            .await
            .expect("일치하는 피어는 통과한다");

            assert_eq!(verified.uid(), UserId::new(501_u32), "UID 보존");
            assert_eq!(
                verified.pid(),
                ProcessId::try_new(2000_u32).expect("PID"),
                "PID 보존"
            );
            assert_eq!(
                verified.parent_pid(),
                expected.expected_parent_pid(),
                "부모 PID 보존"
            );
            assert_eq!(
                verified.executable_path(),
                expected.expected_executable_path(),
                "실행 파일 경로 보존"
            );
            assert_eq!(
                matching.calls(),
                vec![ProcessId::try_new(2000_u32).expect("호출 기록")],
                "조회 포트에 상대 PID가 전달되었다"
            );
        });
    }
}
