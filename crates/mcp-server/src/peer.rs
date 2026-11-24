use std::sync::{Arc, OnceLock};
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
pub(crate) struct ObservedPeerCredentials {
    uid: UserId,
    pid: Option<i32>,
}

impl ObservedPeerCredentials {
    #[must_use]
    pub(crate) const fn new(uid: UserId, pid: Option<i32>) -> Self {
        Self { uid, pid }
    }

    #[must_use]
    pub(crate) const fn uid(self) -> UserId {
        self.uid
    }

    #[must_use]
    pub(crate) const fn pid(self) -> Option<i32> {
        self.pid
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExpectedParentPid {
    Pending,
    Bound(ProcessId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParentPidAlreadyBound;

#[derive(Debug)]
pub(crate) struct ExpectedPeerPolicy {
    uid: UserId,
    parent_pid: OnceLock<ProcessId>,
    executable_path: ExecutablePath,
}

impl ExpectedPeerPolicy {
    #[must_use]
    pub(crate) fn new_pending(uid: UserId, executable_path: ExecutablePath) -> Self {
        Self {
            uid,
            parent_pid: OnceLock::new(),
            executable_path,
        }
    }

    #[must_use]
    pub(crate) fn new_bound(
        uid: UserId,
        parent_pid: ProcessId,
        executable_path: ExecutablePath,
    ) -> Self {
        let slot = OnceLock::new();
        match slot.set(parent_pid) {
            Ok(()) => {}
            Err(_existing) => {}
        }
        Self {
            uid,
            parent_pid: slot,
            executable_path,
        }
    }

    pub(crate) fn bind_parent_pid(
        &self,
        parent_pid: ProcessId,
    ) -> Result<(), ParentPidAlreadyBound> {
        self.parent_pid
            .set(parent_pid)
            .map_err(|_existing| ParentPidAlreadyBound)
    }

    #[must_use]
    pub(crate) const fn expected_uid(&self) -> UserId {
        self.uid
    }

    #[must_use]
    pub(crate) fn expected_parent_pid(&self) -> ExpectedParentPid {
        match self.parent_pid.get().copied() {
            Some(pid) => ExpectedParentPid::Bound(pid),
            None => ExpectedParentPid::Pending,
        }
    }

    #[must_use]
    pub(crate) const fn expected_executable_path(&self) -> &ExecutablePath {
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
    ParentPidUnbound,
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

pub(crate) async fn verify_peer<P: PeerProcessInspector>(
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
    let expected_parent_pid = match expected.expected_parent_pid() {
        ExpectedParentPid::Bound(pid) => pid,
        ExpectedParentPid::Pending => return Err(PeerVerificationFailure::ParentPidUnbound),
    };
    let port = match inspector {
        PeerInspectorMode::Configured(configured) => configured,
        PeerInspectorMode::Unconfigured => return Err(PeerVerificationFailure::InspectorMissing),
    };
    let inspected = tokio::time::timeout(timeout, port.inspect(peer_pid))
        .await
        .map_err(|_elapsed| PeerVerificationFailure::InspectionTimedOut)?
        .map_err(PeerVerificationFailure::InspectionFailed)?;
    if inspected.parent_pid() != expected_parent_pid {
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
        ExpectedParentPid, ExpectedPeerPolicy, ObservedPeerCredentials, ParentPidAlreadyBound,
        PeerInspectorMode, PeerVerificationFailure, UserId, verify_peer,
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
        ExpectedPeerPolicy::new_bound(
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
    fn pending_parent_pid_fails_closed_and_binds_once() {
        runtime().block_on(async {
            let exe =
                ExecutablePath::try_new("/Applications/StockCopilot.app/mcp-bridge").expect("경로");
            let pending = ExpectedPeerPolicy::new_pending(UserId::new(501_u32), exe.clone());
            assert_eq!(
                pending.expected_parent_pid(),
                ExpectedParentPid::Pending,
                "초기 상태는 Pending"
            );
            let parent_pid = ProcessId::try_new(1234_u32).expect("CLI PID");
            let inspector = Arc::new(ScriptedPeerProcessInspector::new([Ok(
                PeerProcessInfo::new(parent_pid, exe.clone()),
            )]));
            let mode = PeerInspectorMode::Configured(inspector);
            let observed = ObservedPeerCredentials::new(UserId::new(501_u32), Some(2000_i32));
            assert_eq!(
                verify_peer(observed, &pending, &mode, Duration::from_secs(1_u64)).await,
                Err(PeerVerificationFailure::ParentPidUnbound),
                "부모 PID 결합 전에는 닫힌 실패로 거부한다"
            );
            assert_eq!(
                pending.bind_parent_pid(parent_pid),
                Ok(()),
                "최초 1회 결합 성공"
            );
            assert_eq!(
                pending.bind_parent_pid(ProcessId::try_new(9999_u32).expect("다른 PID")),
                Err(ParentPidAlreadyBound),
                "재결합 시도는 거부한다"
            );
            let verified = verify_peer(observed, &pending, &mode, Duration::from_secs(1_u64))
                .await
                .expect("결합 후 일치 상대 허용");
            assert_eq!(verified.parent_pid(), parent_pid, "결합된 부모 PID 일치");
        });
    }

    #[test]
    fn peer_verification_rejects_lookup_errors_and_mismatches() {
        runtime().block_on(async {
            let expected = policy();
            let timeout = Duration::from_secs(1_u64);
            let observed = ObservedPeerCredentials::new(UserId::new(501_u32), Some(2000_i32));
            let expected_parent = match expected.expected_parent_pid() {
                ExpectedParentPid::Bound(pid) => pid,
                ExpectedParentPid::Pending => panic!("테스트 정책은 결합된 상태여야 한다"),
            };
            let failing = Arc::new(ScriptedPeerProcessInspector::new([
                Err(PeerProcessLookupError::Unavailable),
                Err(PeerProcessLookupError::NotFound),
                Err(PeerProcessLookupError::PermissionDenied),
                Ok(PeerProcessInfo::new(
                    ProcessId::try_new(9999_u32).expect("다른 부모"),
                    expected.expected_executable_path().clone(),
                )),
                Ok(PeerProcessInfo::new(
                    expected_parent,
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
                    "부모 PID 불일치 거부",
                ),
                (
                    PeerVerificationFailure::ExecutablePathMismatch,
                    "실행 파일 경로 불일치 거부",
                ),
            ];
            for (expected_err, message) in cases {
                assert_eq!(
                    verify_peer(observed, &expected, &mode, timeout).await,
                    Err(expected_err),
                    "{message}"
                );
            }
        });
    }

    #[test]
    fn peer_verification_enforces_timeout_and_accepts_matching_peer() {
        runtime().block_on(async {
            let expected = policy();
            let observed = ObservedPeerCredentials::new(UserId::new(501_u32), Some(2000_i32));
            let slow = PeerInspectorMode::Configured(Arc::new(SlowInspector));
            assert_eq!(
                verify_peer(observed, &expected, &slow, Duration::from_millis(20_u64)).await,
                Err(PeerVerificationFailure::InspectionTimedOut),
                "조회 제한 시간 초과 시 닫힌 실패"
            );

            let expected_parent = match expected.expected_parent_pid() {
                ExpectedParentPid::Bound(pid) => pid,
                ExpectedParentPid::Pending => panic!("테스트 정책은 결합된 상태여야 한다"),
            };
            let ok_inspector = Arc::new(ScriptedPeerProcessInspector::new([Ok(
                PeerProcessInfo::new(expected_parent, expected.expected_executable_path().clone()),
            )]));
            let verified = verify_peer(
                observed,
                &expected,
                &PeerInspectorMode::Configured(Arc::clone(&ok_inspector)),
                Duration::from_secs(1_u64),
            )
            .await
            .expect("일치하는 상대 허용");
            assert_eq!(verified.uid(), UserId::new(501_u32), "UID 보존");
            assert_eq!(
                verified.pid(),
                ProcessId::try_new(2000_u32).expect("PID"),
                "PID 보존"
            );
            assert_eq!(verified.parent_pid(), expected_parent, "부모 PID 보존");
            assert_eq!(
                verified.executable_path(),
                expected.expected_executable_path(),
                "실행 파일 경로 보존"
            );
            assert_eq!(
                ok_inspector.calls(),
                vec![ProcessId::try_new(2000_u32).expect("기록 PID")],
                "조회 포트에 상대 PID가 전달됐다"
            );
        });
    }
}
