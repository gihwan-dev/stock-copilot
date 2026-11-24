use std::fmt;

use domain::{
    UntrustedText, WebSearchCandidateQuery, WebSearchInspection, WebSearchQueryDigest,
    WebSearchSensitiveContext, inspect_web_search_query,
};

use crate::{
    AuditEntry, AuditKind, AuditLog, Clock, RecordOutcome, StoredUserMessage, SystemPortError,
    WebSearchAuditRecord,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebSearchGuardError {
    AuditFailed(SystemPortError),
}

#[derive(Clone, PartialEq, Eq)]
pub struct UserDirectWebSearchQuery {
    message: StoredUserMessage,
    digest: WebSearchQueryDigest,
}

impl UserDirectWebSearchQuery {
    #[must_use]
    pub const fn message(&self) -> &StoredUserMessage {
        &self.message
    }

    #[must_use]
    pub const fn text(&self) -> &UntrustedText {
        self.message.text()
    }

    #[must_use]
    pub const fn digest(&self) -> &WebSearchQueryDigest {
        &self.digest
    }
}

impl fmt::Debug for UserDirectWebSearchQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserDirectWebSearchQuery")
            .field("message_id", self.message.id())
            .field("digest", &self.digest)
            .finish_non_exhaustive()
    }
}

pub struct WebSearchGuard<'a, A: AuditLog> {
    clock: &'a dyn Clock,
    audit_log: &'a A,
}

impl<A: AuditLog> fmt::Debug for WebSearchGuard<'_, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebSearchGuard").finish_non_exhaustive()
    }
}

impl<'a, A: AuditLog> WebSearchGuard<'a, A> {
    #[must_use]
    pub const fn new(clock: &'a dyn Clock, audit_log: &'a A) -> Self {
        Self { clock, audit_log }
    }

    pub async fn inspect_code_query(
        &self,
        candidate: &WebSearchCandidateQuery,
        context: &WebSearchSensitiveContext,
    ) -> Result<WebSearchInspection, WebSearchGuardError> {
        let inspection = inspect_web_search_query(candidate, context);
        let record = WebSearchAuditRecord::from_inspection(&inspection);
        let entry = AuditEntry::new(self.clock.now(), AuditKind::WebSearchChecked(record));
        match self
            .audit_log
            .append(&entry)
            .await
            .map_err(WebSearchGuardError::AuditFailed)?
        {
            RecordOutcome::Recorded | RecordOutcome::AlreadyRecorded => {}
        }
        Ok(inspection)
    }

    pub async fn record_user_direct_query(
        &self,
        message: &StoredUserMessage,
    ) -> Result<UserDirectWebSearchQuery, WebSearchGuardError> {
        let digest = WebSearchQueryDigest::of_untrusted(message.text());
        let record = WebSearchAuditRecord::user_direct(digest.clone());
        let entry = AuditEntry::new(self.clock.now(), AuditKind::WebSearchChecked(record));
        match self
            .audit_log
            .append(&entry)
            .await
            .map_err(WebSearchGuardError::AuditFailed)?
        {
            RecordOutcome::Recorded | RecordOutcome::AlreadyRecorded => {}
        }
        Ok(UserDirectWebSearchQuery {
            message: message.clone(),
            digest,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::future::{Future, ready};
    use std::sync::RwLock;
    use std::task::{Context, Poll, Waker};

    use domain::{
        Nanosecond, OffsetSeconds, Timestamp, UnixSeconds, UntrustedText, WebSearchCandidateQuery,
        WebSearchInspection, WebSearchRejectionReason, WebSearchSensitiveContext,
    };

    use crate::{
        AuditEntry, AuditIntegrity, AuditKind, AuditLog, Clock, RecordOutcome, StoredUserMessage,
        SystemFailure, SystemPort, SystemPortError, UserMessageId, WebSearchAuditError,
        WebSearchAuditOutcome, WebSearchAuditRecord, WebSearchGuard, WebSearchGuardError,
        WebSearchQueryMode,
    };

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("expected ready"),
        }
    }

    struct FixedClock(Timestamp);
    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            self.0
        }
    }

    enum AuditReply {
        Ok,
        Err,
    }

    struct StubAudit {
        reply: AuditReply,
        entries: RwLock<Vec<AuditEntry>>,
    }

    impl AuditLog for StubAudit {
        fn append(
            &self,
            entry: &AuditEntry,
        ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send {
            match self.reply {
                AuditReply::Ok => {
                    self.entries.write().expect("lock").push(entry.clone());
                    ready(Ok(RecordOutcome::Recorded))
                }
                AuditReply::Err => ready(Err(SystemPortError::new(
                    SystemPort::AuditLog,
                    SystemFailure::RepositoryIntegrity,
                ))),
            }
        }

        fn verify(&self) -> impl Future<Output = Result<AuditIntegrity, SystemPortError>> + Send {
            ready(Ok(AuditIntegrity::Verified))
        }
    }

    fn fixed_time() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(1_800_000_000_i64),
            Nanosecond::new(0_u32),
            OffsetSeconds::new(0_i32),
        )
        .expect("timestamp")
    }

    #[test]
    fn code_query_approval_and_rejection_are_audited_without_plaintext_leak() {
        let clock = FixedClock(fixed_time());
        let audit = StubAudit {
            reply: AuditReply::Ok,
            entries: RwLock::new(Vec::new()),
        };
        let guard = WebSearchGuard::new(&clock, &audit);
        assert_eq!(format!("{guard:?}"), "WebSearchGuard { .. }");

        let ok_res = run(guard.inspect_code_query(
            &WebSearchCandidateQuery::new("삼성전자 2026년 3분기 실적"),
            &WebSearchSensitiveContext::new(),
        ))
        .expect("audit ok");
        let WebSearchInspection::Approved(approved) = ok_res else {
            panic!("expected approved");
        };
        assert_eq!(approved.text().as_str(), "삼성전자 2026년 3분기 실적");

        let rej_res = run(guard.inspect_code_query(
            &WebSearchCandidateQuery::new("삼성전자 계좌 12345678"),
            &WebSearchSensitiveContext::new(),
        ))
        .expect("audit rejected");
        let WebSearchInspection::Rejected(rejected) = rej_res else {
            panic!("expected rejected");
        };
        assert!(
            rejected
                .reasons()
                .contains(&WebSearchRejectionReason::MultiDigitNumber)
        );
        assert!(
            rejected
                .reasons()
                .contains(&WebSearchRejectionReason::MoneyQuantityOrAccountTerm)
        );

        let entries = audit.entries.read().expect("lock");
        let [first_entry, second_entry] = entries.as_slice() else {
            panic!("expected 2 audit entries");
        };
        let debug_all = format!("{entries:?}");
        assert!(!debug_all.contains("12345678"), "{debug_all}");
        assert!(!debug_all.contains("삼성전자"), "{debug_all}");
        let AuditKind::WebSearchChecked(first) = first_entry.kind() else {
            panic!("expected web search record");
        };
        assert_eq!(first.mode(), WebSearchQueryMode::CodeGenerated);
        assert_eq!(first.digest(), approved.digest());
        assert_eq!(first.outcome(), &WebSearchAuditOutcome::Approved);
        let AuditKind::WebSearchChecked(second) = second_entry.kind() else {
            panic!("expected web search record");
        };
        assert_eq!(second.mode(), WebSearchQueryMode::CodeGenerated);
        assert_eq!(second.digest(), rejected.digest());
        assert_eq!(
            second.outcome(),
            &WebSearchAuditOutcome::Rejected(rejected.reasons().to_vec())
        );
    }

    #[test]
    fn guard_fails_closed_when_audit_append_fails_and_preserves_user_direct_bytes() {
        let clock = FixedClock(fixed_time());
        let failing_audit = StubAudit {
            reply: AuditReply::Err,
            entries: RwLock::new(Vec::new()),
        };
        let failing_guard = WebSearchGuard::new(&clock, &failing_audit);
        let code_err = run(failing_guard.inspect_code_query(
            &WebSearchCandidateQuery::new("삼성전자 실적"),
            &WebSearchSensitiveContext::new(),
        ))
        .expect_err("must fail closed");
        assert_eq!(
            code_err,
            WebSearchGuardError::AuditFailed(SystemPortError::new(
                SystemPort::AuditLog,
                SystemFailure::RepositoryIntegrity,
            ))
        );

        let raw_user_text = "내 계좌 12345678 삼성전자 500주 매도 시점 알려줘";
        let stored = StoredUserMessage::from_store_result(
            UserMessageId::try_new("user-msg-1").expect("id"),
            UntrustedText::try_new(raw_user_text).expect("text"),
        );
        let user_err = run(failing_guard.record_user_direct_query(&stored))
            .expect_err("user direct must also fail closed when audit fails");
        assert_eq!(user_err, code_err);

        let ok_audit = StubAudit {
            reply: AuditReply::Ok,
            entries: RwLock::new(Vec::new()),
        };
        let ok_guard = WebSearchGuard::new(&clock, &ok_audit);
        let direct = run(ok_guard.record_user_direct_query(&stored)).expect("user direct ok");
        assert_eq!(direct.text().as_untrusted_str(), raw_user_text);
        assert_eq!(direct.message(), &stored);
        let direct_debug = format!("{direct:?}");
        assert!(
            direct_debug.contains("UserDirectWebSearchQuery"),
            "{direct_debug}"
        );
        assert!(!direct_debug.contains("12345678"), "{direct_debug}");
        let entries = ok_audit.entries.read().expect("lock");
        let [only_entry] = entries.as_slice() else {
            panic!("expected 1 audit entry");
        };
        let AuditKind::WebSearchChecked(rec) = only_entry.kind() else {
            panic!("expected web search record");
        };
        assert_eq!(rec.mode(), WebSearchQueryMode::UserDirectMessage);
        assert_eq!(rec.digest(), direct.digest());
        assert_eq!(
            rec.outcome(),
            &WebSearchAuditOutcome::BypassedForUserMessage
        );
    }

    #[test]
    fn web_search_audit_record_rejects_empty_and_canonicalizes_reasons() {
        let digest = domain::WebSearchQueryDigest::of_query(&WebSearchCandidateQuery::new("q"));
        assert_eq!(
            WebSearchAuditRecord::try_code_rejected(digest.clone(), Vec::new()),
            Err(WebSearchAuditError::EmptyRejectionReasons)
        );
        let deduped = WebSearchAuditRecord::try_code_rejected(
            digest,
            vec![
                WebSearchRejectionReason::MultiDigitNumber,
                WebSearchRejectionReason::EmptyQuery,
                WebSearchRejectionReason::MultiDigitNumber,
            ],
        )
        .expect("canonical deduped reasons");
        assert_eq!(
            deduped.outcome(),
            &WebSearchAuditOutcome::Rejected(vec![
                WebSearchRejectionReason::EmptyQuery,
                WebSearchRejectionReason::MultiDigitNumber,
            ])
        );
    }
}
