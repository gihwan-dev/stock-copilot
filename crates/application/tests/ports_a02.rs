#[cfg(test)]
mod tests {
    use application::{
        AuditEntry, AuditIntegrity, AuditKind, BugDiagnostic, ClassifiedError, ErrorClass,
        Notification, OrderConfirmation, RetryDelay, RiskChangeConfirmation, SecretKey,
        SecretValue, SystemDiagnostic, SystemFailure, SystemPort, SystemPortError, UserAction,
    };
    use domain::{
        ClientOrderId, InstrumentId, InvestorProfile, Nanosecond, OffsetSeconds, ProposalId,
        RiskAppetite, RiskChange, SpanSeconds, SyncOutcome, SyncRecord, TimeSpan, Timestamp,
        UnixSeconds,
    };

    fn instant() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(123),
            Nanosecond::new(456),
            OffsetSeconds::new(0),
        )
        .expect("고정 시각")
    }

    fn expected_class(cause: SystemFailure) -> ErrorClass {
        match cause {
            SystemFailure::PermissionDenied => ErrorClass::UserAction(UserAction::CheckPermissions),
            SystemFailure::Unavailable(delay) => ErrorClass::Wait(delay),
            SystemFailure::InvalidData => {
                ErrorClass::Bug(BugDiagnostic::System(SystemDiagnostic::InvalidStoredData))
            }
            SystemFailure::RepositoryIntegrity => {
                ErrorClass::Bug(BugDiagnostic::System(SystemDiagnostic::RepositoryIntegrity))
            }
            SystemFailure::SecretIntegrity => {
                ErrorClass::Bug(BugDiagnostic::System(SystemDiagnostic::SecretIntegrity))
            }
        }
    }

    fn expected_port(port: SystemPort) -> &'static str {
        match port {
            SystemPort::Repository => "repository",
            SystemPort::AuditLog => "audit",
            SystemPort::SecretStore => "secret",
            SystemPort::Notifier => "notifier",
        }
    }

    #[test]
    fn audit_integrity_has_exactly_two_distinct_states() {
        fn expected(value: AuditIntegrity) -> &'static str {
            match value {
                AuditIntegrity::Verified => "verified",
                AuditIntegrity::Tampered => "tampered",
            }
        }
        assert_eq!(expected(AuditIntegrity::Verified), "verified");
        assert_eq!(expected(AuditIntegrity::Tampered), "tampered");
        assert_ne!(AuditIntegrity::Verified, AuditIntegrity::Tampered);
    }

    #[test]
    fn closed_failures_preserve_port_cause_and_exact_wait() {
        let wait = RetryDelay::Known(
            TimeSpan::try_new(SpanSeconds::new(17), Nanosecond::new(321)).expect("대기"),
        );
        for (port, name) in [
            (SystemPort::Repository, "repository"),
            (SystemPort::AuditLog, "audit"),
            (SystemPort::SecretStore, "secret"),
            (SystemPort::Notifier, "notifier"),
        ] {
            assert_eq!(expected_port(port), name);
            for cause in [
                SystemFailure::PermissionDenied,
                SystemFailure::Unavailable(wait),
                SystemFailure::Unavailable(RetryDelay::Unknown),
                SystemFailure::InvalidData,
                SystemFailure::RepositoryIntegrity,
                SystemFailure::SecretIntegrity,
            ] {
                let error = SystemPortError::new(port, cause);
                assert_eq!(error.port(), port);
                assert_eq!(error.failure(), cause);
                let expected = expected_class(cause);
                assert_eq!(error.class(), expected);
            }
        }
    }

    #[test]
    fn secret_access_is_explicit_and_preserves_fake_bytes() {
        for fake in [
            b"".as_slice(),
            b"fake-secret".as_slice(),
            &[255, 0, 128],
            b"  fake-secret  ".as_slice(),
        ] {
            let value = SecretValue::new(fake.to_vec());
            assert_eq!(value.as_exposed_secret(), fake);
        }
    }

    #[test]
    fn confirmation_has_exactly_two_requests_and_three_distinct_results() {
        fn result_kind(result: OrderConfirmation) -> AuditIntegrity {
            match result {
                OrderConfirmation::Confirmed => AuditIntegrity::Verified,
                OrderConfirmation::Denied | OrderConfirmation::Unavailable => {
                    AuditIntegrity::Tampered
                }
            }
        }
        assert_eq!(
            result_kind(OrderConfirmation::Confirmed),
            AuditIntegrity::Verified
        );
        assert_eq!(
            result_kind(OrderConfirmation::Denied),
            AuditIntegrity::Tampered
        );
        assert_eq!(
            result_kind(OrderConfirmation::Unavailable),
            AuditIntegrity::Tampered
        );
        assert_ne!(OrderConfirmation::Denied, OrderConfirmation::Unavailable);
    }

    #[test]
    fn secret_keys_and_notifications_have_no_open_text_payload() {
        fn key(value: SecretKey) -> &'static str {
            match value {
                SecretKey::TossClientId => "client-id",
                SecretKey::TossClientSecret => "client-secret",
                SecretKey::TossAccessToken => "access-token",
                SecretKey::DatabaseEncryptionKey => "database-key",
                SecretKey::AuditHmacKey => "audit-hmac",
                SecretKey::DartApiKey => "dart-key",
                SecretKey::FredApiKey => "fred-key",
            }
        }
        fn notification(value: Notification) -> &'static str {
            match value {
                Notification::NewSignal => "signal",
                Notification::UrgentDisclosure => "disclosure",
                Notification::BriefingFinished => "briefing",
            }
        }
        for (value, expected) in [
            (SecretKey::TossClientId, "client-id"),
            (SecretKey::TossClientSecret, "client-secret"),
            (SecretKey::TossAccessToken, "access-token"),
            (SecretKey::DatabaseEncryptionKey, "database-key"),
            (SecretKey::AuditHmacKey, "audit-hmac"),
            (SecretKey::DartApiKey, "dart-key"),
            (SecretKey::FredApiKey, "fred-key"),
        ] {
            assert_eq!(key(value), expected);
        }
        for (value, expected) in [
            (Notification::NewSignal, "signal"),
            (Notification::UrgentDisclosure, "disclosure"),
            (Notification::BriefingFinished, "briefing"),
        ] {
            assert_eq!(notification(value), expected);
        }
    }

    fn risk_changes() -> Vec<RiskChange> {
        let profile = InvestorProfile::default();
        vec![
            RiskChange::EnableOrders,
            RiskChange::AllowMarketOrders,
            RiskChange::AllowHours,
            RiskChange::RelaxWeightCaps(profile.weight_caps()),
            RiskChange::IncreaseRiskAppetite(RiskAppetite::Aggressive),
        ]
    }

    #[test]
    fn risk_requests_keep_target_values() {
        fn target(change: &RiskChange) -> RiskChange {
            match change {
                RiskChange::EnableOrders => RiskChange::EnableOrders,
                RiskChange::AllowMarketOrders => RiskChange::AllowMarketOrders,
                RiskChange::AllowHours => RiskChange::AllowHours,
                RiskChange::RelaxWeightCaps(caps) => RiskChange::RelaxWeightCaps(*caps),
                RiskChange::IncreaseRiskAppetite(appetite) => {
                    RiskChange::IncreaseRiskAppetite(*appetite)
                }
            }
        }
        for change in risk_changes() {
            assert_eq!(target(&change), change);
        }
    }

    fn audit_kinds() -> Vec<AuditKind> {
        let proposal =
            ProposalId::new(ClientOrderId::try_new("fixture-proposal").expect("제안 키"));
        let instrument = InstrumentId::try_new("fixture-instrument").expect("종목 키");
        let order = ClientOrderId::try_new("fixture-order").expect("주문 키");
        let mut kinds = vec![
            AuditKind::EstimateSubmitted(instrument.clone()),
            AuditKind::SignalRaised(instrument),
            AuditKind::ValidationRecorded(proposal.clone()),
            AuditKind::ApprovalRequested(proposal.clone()),
            AuditKind::ProposalSkipped(proposal.clone()),
            AuditKind::SubmissionPrepared(order.clone()),
            AuditKind::SubmissionResponseRecorded(order),
        ];
        for outcome in [
            OrderConfirmation::Confirmed,
            OrderConfirmation::Denied,
            OrderConfirmation::Unavailable,
        ] {
            kinds.push(AuditKind::PresenceRecorded {
                proposal: proposal.clone(),
                outcome,
            });
        }
        for change in risk_changes() {
            for outcome in [
                RiskChangeConfirmation::Confirmed,
                RiskChangeConfirmation::Denied,
                RiskChangeConfirmation::Unavailable,
            ] {
                kinds.push(AuditKind::RiskChangeConfirmationRecorded {
                    change: change.clone(),
                    outcome,
                });
            }
            kinds.push(AuditKind::RiskSettingsChanged(change));
        }
        kinds
    }

    #[test]
    fn audit_entries_keep_all_kinds() {
        fn audit_kind(kind: &AuditKind) -> AuditKind {
            match kind {
                AuditKind::EstimateSubmitted(id) => AuditKind::EstimateSubmitted(id.clone()),
                AuditKind::SignalRaised(id) => AuditKind::SignalRaised(id.clone()),
                AuditKind::ValidationRecorded(id) => AuditKind::ValidationRecorded(id.clone()),
                AuditKind::ApprovalRequested(id) => AuditKind::ApprovalRequested(id.clone()),
                AuditKind::PresenceRecorded { proposal, outcome } => AuditKind::PresenceRecorded {
                    proposal: proposal.clone(),
                    outcome: *outcome,
                },
                AuditKind::ProposalSkipped(id) => AuditKind::ProposalSkipped(id.clone()),
                AuditKind::SubmissionPrepared(id) => AuditKind::SubmissionPrepared(id.clone()),
                AuditKind::SubmissionResponseRecorded(id) => {
                    AuditKind::SubmissionResponseRecorded(id.clone())
                }
                AuditKind::RiskChangeConfirmationRecorded { change, outcome } => {
                    AuditKind::RiskChangeConfirmationRecorded {
                        change: change.clone(),
                        outcome: *outcome,
                    }
                }
                AuditKind::RiskSettingsChanged(change) => {
                    AuditKind::RiskSettingsChanged(change.clone())
                }
            }
        }
        for kind in audit_kinds() {
            let entry = AuditEntry::new(instant(), kind.clone());
            assert_eq!(entry.at(), instant());
            assert_eq!(entry.kind(), &kind);
            assert_eq!(audit_kind(entry.kind()), kind);
        }
    }

    #[test]
    fn failed_syncs_remain_distinct_from_empty_or_partial_success() {
        let value = InvestorProfile::default();
        for outcome in [
            SyncOutcome::Complete(value.clone()),
            SyncOutcome::Partial(value),
            SyncOutcome::Failed,
            SyncOutcome::AccessBlocked,
        ] {
            let record = SyncRecord::new(instant(), outcome.clone());
            assert_eq!(record.recorded_at(), instant());
            assert_eq!(record.outcome(), &outcome);
            let expected = match &outcome {
                SyncOutcome::Complete(profile) => SyncOutcome::Complete(profile.clone()),
                SyncOutcome::Partial(profile) => SyncOutcome::Partial(profile.clone()),
                SyncOutcome::Failed => SyncOutcome::Failed,
                SyncOutcome::AccessBlocked => SyncOutcome::AccessBlocked,
            };
            assert_eq!(record.outcome(), &expected);
        }
    }
}
