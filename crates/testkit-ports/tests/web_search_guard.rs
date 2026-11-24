#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    use application::{
        AuditKind, StoredUserMessage, SystemFailure, SystemPort, SystemPortError, UserMessageId,
        WebSearchAuditOutcome, WebSearchGuard, WebSearchGuardError, WebSearchQueryMode,
    };
    use domain::{
        MaskedAccountNumber, Nanosecond, OffsetSeconds, SensitiveAccountNumber,
        SensitivePersonalName, Timestamp, UnixSeconds, UntrustedText, WebSearchCandidateQuery,
        WebSearchInspection, WebSearchRejectionReason, WebSearchSensitiveContext,
    };
    use testkit_ports::{SteppedClock, memory::MemoryReply, system_fakes::MemoryAuditLog};

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("expected immediate completion"),
        }
    }

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0_u32),
            OffsetSeconds::new(0_i32),
        )
        .expect("timestamp")
    }

    fn assert_recorded_entries(audit: &MemoryAuditLog) {
        let entries = audit.entries();
        let [first_entry, second_entry, third_entry] = entries.as_slice() else {
            panic!("expected 3 entries");
        };
        let serialized = format!("{entries:?}");
        for secret in ["김철수", "111-22-333444", "333444", "삼성전자"] {
            assert!(!serialized.contains(secret), "{serialized}");
        }
        assert_eq!(first_entry.at(), at(1_800_000_000_i64));
        assert_eq!(second_entry.at(), at(1_800_000_010_i64));
        assert_eq!(third_entry.at(), at(1_800_000_020_i64));
        let AuditKind::WebSearchChecked(first) = first_entry.kind() else {
            panic!("first entry");
        };
        assert_eq!(first.mode(), WebSearchQueryMode::CodeGenerated);
        assert_eq!(first.outcome(), &WebSearchAuditOutcome::Approved);
        let AuditKind::WebSearchChecked(second) = second_entry.kind() else {
            panic!("second entry");
        };
        assert_eq!(second.mode(), WebSearchQueryMode::CodeGenerated);
        match second.outcome() {
            WebSearchAuditOutcome::Rejected(reasons) => {
                assert!(reasons.contains(&WebSearchRejectionReason::SensitiveContextMatch));
            }
            WebSearchAuditOutcome::Approved | WebSearchAuditOutcome::BypassedForUserMessage => {
                panic!("expected rejected audit outcome");
            }
        }
        let AuditKind::WebSearchChecked(third) = third_entry.kind() else {
            panic!("third entry");
        };
        assert_eq!(third.mode(), WebSearchQueryMode::UserDirectMessage);
        assert_eq!(
            third.outcome(),
            &WebSearchAuditOutcome::BypassedForUserMessage
        );
    }

    #[test]
    fn guard_audits_approvals_rejections_and_user_direct_queries_in_memory_log() {
        let clock = SteppedClock::new(at(1_800_000_000_i64));
        let audit = MemoryAuditLog::default();
        audit.script_append().push(MemoryReply::Memory);
        audit.script_append().push(MemoryReply::Memory);
        audit.script_append().push(MemoryReply::Memory);
        let guard = WebSearchGuard::new(&clock, &audit);

        let ctx = WebSearchSensitiveContext::new()
            .with_account_number(SensitiveAccountNumber::try_new("111-22-333444").expect("acct"))
            .with_masked_account(MaskedAccountNumber::try_new("555556666").expect("masked"))
            .with_personal_name(SensitivePersonalName::try_new("김철수").expect("name"));

        let approved_res = run(guard.inspect_code_query(
            &WebSearchCandidateQuery::new("삼성전자 2026년 3분기 실적"),
            &ctx,
        ))
        .expect("approved audit");
        let WebSearchInspection::Approved(approved) = approved_res else {
            panic!("expected approved");
        };
        assert_eq!(approved.text().as_str(), "삼성전자 2026년 3분기 실적");

        clock.advance_to(at(1_800_000_010_i64)).expect("advance");
        let rejected_res = run(guard.inspect_code_query(
            &WebSearchCandidateQuery::new("삼성전자 김철수 333444"),
            &ctx,
        ))
        .expect("rejected audit");
        let WebSearchInspection::Rejected(rejected) = rejected_res else {
            panic!("expected rejected");
        };
        assert!(
            rejected
                .reasons()
                .contains(&WebSearchRejectionReason::SensitiveContextMatch)
        );
        assert!(
            rejected
                .reasons()
                .contains(&WebSearchRejectionReason::MultiDigitNumber)
        );

        clock.advance_to(at(1_800_000_020_i64)).expect("advance");
        let exact_user_sentence = "내 계좌 111-22-333444 김철수 삼성전자 100주 전망";
        let stored = StoredUserMessage::from_store_result(
            UserMessageId::try_new("msg-42").expect("msg id"),
            UntrustedText::try_new(exact_user_sentence).expect("text"),
        );
        let direct = run(guard.record_user_direct_query(&stored)).expect("user direct audit");
        assert_eq!(
            direct.text().as_untrusted_str().as_bytes(),
            exact_user_sentence.as_bytes()
        );
        assert_recorded_entries(&audit);
    }

    #[test]
    fn guard_refuses_query_when_memory_audit_log_fails() {
        let clock = SteppedClock::new(at(1_800_000_000_i64));
        let audit = MemoryAuditLog::default();
        let err = SystemPortError::new(SystemPort::AuditLog, SystemFailure::PermissionDenied);
        audit.script_append().push(MemoryReply::Return(Err(err)));
        let guard = WebSearchGuard::new(&clock, &audit);

        let result = run(guard.inspect_code_query(
            &WebSearchCandidateQuery::new("삼성전자 실적"),
            &WebSearchSensitiveContext::new(),
        ));
        assert_eq!(result, Err(WebSearchGuardError::AuditFailed(err)));
        assert_eq!(audit.entries(), Vec::new());
    }
}
