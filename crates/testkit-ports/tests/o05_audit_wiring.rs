#[cfg(test)]
mod tests {
    use application::{
        AuditEntry, AuditEventRecorder, AuditEventSink, AuditKind, ChangeRiskSettings,
        EventBusBuilder, RiskAudit, RiskChangeConfirmation, RiskSettingsChangeOutcome,
        SubscriberFailure,
    };
    use domain::{
        ClientOrderId, DomainEvent, HoursPermissionState, InstrumentId, Nanosecond, OffsetSeconds,
        OrderPermission, ProposalId, RiskAppetite, RiskChange, RiskSettings,
        SavedMarketOrderSetting, Timestamp, UnixSeconds, WeightCaps,
    };
    use std::sync::{Arc, RwLock};
    use std::task::{Context, Poll, Waker};
    use testkit_ports::memory::{MemoryReply, MemorySettingsRepository};
    use testkit_ports::system_fakes::MemoryAuditLog;
    use testkit_ports::{ScriptedPresence, SteppedClock};

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("즉시 완료되어야 한다"),
        }
    }

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(32_400),
        )
        .expect("시각")
    }

    #[derive(Clone, Default)]
    struct CollectingSink(Arc<RwLock<Vec<AuditEntry>>>);

    impl CollectingSink {
        fn entries(&self) -> Vec<AuditEntry> {
            match self.0.read() {
                Ok(guard) => guard.clone(),
                Err(poisoned) => poisoned.into_inner().clone(),
            }
        }
    }

    impl AuditEventSink for CollectingSink {
        fn enqueue(&self, entry: AuditEntry) -> Result<(), SubscriberFailure> {
            match self.0.write() {
                Ok(mut guard) => guard.push(entry),
                Err(poisoned) => poisoned.into_inner().push(entry),
            }
            Ok(())
        }
    }

    fn audit_log(slots: usize) -> MemoryAuditLog {
        let log = MemoryAuditLog::default();
        for _ in 0..slots {
            log.script_append().push(MemoryReply::Memory);
        }
        log
    }

    fn kinds(log: &MemoryAuditLog) -> Vec<AuditKind> {
        log.entries()
            .iter()
            .map(|entry| entry.kind().clone())
            .collect()
    }

    fn repository() -> MemorySettingsRepository {
        let repo = MemorySettingsRepository::default();
        for _ in 0_u8..8 {
            repo.script_store_risk_settings().push(MemoryReply::Memory);
            repo.script_risk_settings().push(MemoryReply::Memory);
            repo.script_profile().push(MemoryReply::Memory);
        }
        repo
    }

    fn settings(orders: OrderPermission) -> RiskSettings {
        RiskSettings::new(
            orders,
            SavedMarketOrderSetting::Disabled,
            HoursPermissionState::Disabled,
            WeightCaps::default(),
            RiskAppetite::Neutral,
        )
    }

    #[test]
    fn estimate_signal_and_skip_events_each_leave_one_audit_entry() {
        let clock = Arc::new(SteppedClock::new(at(1_775_091_600)));
        let sink = CollectingSink::default();
        let mut builder = EventBusBuilder::default();
        let _subscription = builder.subscribe(
            AuditEventRecorder::<CollectingSink>::kinds(),
            AuditEventRecorder::new(clock, sink.clone()),
        );
        let bus = builder.build();
        let instrument = InstrumentId::try_new("kr-069500").expect("종목");
        let proposal = ProposalId::new(ClientOrderId::try_new("o05-skip-1").expect("키"));
        for event in [
            DomainEvent::EstimateSubmitted(instrument.clone()),
            DomainEvent::SignalRaised(instrument.clone()),
            DomainEvent::ProposalSkipped(proposal.clone()),
            DomainEvent::PriceUpdated(instrument.clone()),
            DomainEvent::SettingsChanged,
        ] {
            assert_eq!(bus.publish(event).failures, Vec::new());
        }
        assert_eq!(
            sink.entries(),
            vec![
                AuditEntry::new(
                    at(1_775_091_600),
                    AuditKind::EstimateSubmitted(instrument.clone())
                ),
                AuditEntry::new(at(1_775_091_600), AuditKind::SignalRaised(instrument)),
                AuditEntry::new(at(1_775_091_600), AuditKind::ProposalSkipped(proposal)),
            ]
        );
    }

    #[test]
    fn risk_change_records_confirmation_and_change_once_each() {
        let repo = repository();
        let presence = ScriptedPresence::new([], [RiskChangeConfirmation::Confirmed]);
        let bus = EventBusBuilder::default().build();
        let log = audit_log(8);
        let clock = SteppedClock::new(at(1_775_091_600));
        let use_case =
            ChangeRiskSettings::with_audit(&repo, &presence, &bus, RiskAudit::new(&log, &clock));
        let outcome = run(use_case.execute(settings(OrderPermission::Enabled))).expect("변경");
        assert!(matches_changed(&outcome), "{outcome:?}");
        assert_eq!(
            kinds(&log),
            vec![
                AuditKind::RiskChangeConfirmationRecorded {
                    change: RiskChange::EnableOrders,
                    outcome: RiskChangeConfirmation::Confirmed,
                },
                AuditKind::RiskSettingsChanged(RiskChange::EnableOrders),
            ]
        );
    }

    #[test]
    fn denied_risk_change_records_only_the_confirmation_and_keeps_orders_off() {
        let repo = repository();
        let presence = ScriptedPresence::new([], [RiskChangeConfirmation::Denied]);
        let bus = EventBusBuilder::default().build();
        let log = audit_log(8);
        let clock = SteppedClock::new(at(1_775_091_600));
        let use_case =
            ChangeRiskSettings::with_audit(&repo, &presence, &bus, RiskAudit::new(&log, &clock));
        let outcome = run(use_case.execute(settings(OrderPermission::Enabled))).expect("거부");
        assert_eq!(outcome, RiskSettingsChangeOutcome::Denied);
        assert_eq!(
            kinds(&log),
            vec![AuditKind::RiskChangeConfirmationRecorded {
                change: RiskChange::EnableOrders,
                outcome: RiskChangeConfirmation::Denied,
            }]
        );
        assert_eq!(
            run(application::SettingsRepository::risk_settings(&repo)).expect("저장"),
            None,
            "켜기 거부 뒤 저장 0건"
        );
    }

    fn matches_changed(outcome: &RiskSettingsChangeOutcome) -> bool {
        match outcome {
            RiskSettingsChangeOutcome::Changed(report) => report.failures.is_empty(),
            RiskSettingsChangeOutcome::Unchanged
            | RiskSettingsChangeOutcome::Denied
            | RiskSettingsChangeOutcome::Unavailable
            | RiskSettingsChangeOutcome::ProfileRejected(_) => false,
        }
    }
}
