mod support;

#[cfg(test)]
mod tests {
    use application::{
        CheckTossSpec, Clock, EventBusBuilder, EventSubscriber, HttpLastModified,
        SchemaReviewState, SpecChangeReason, SpecDocumentSnapshot, SpecDriftStatus, SpecFetchError,
        SpecHash, SpecVersionLabel, SubscriberFailure, TossSpecBaseline, TossSpecObservation,
        TossSpecStatusRepository,
    };
    use broker_toss::{
        PINNED_LAST_MODIFIED, PINNED_NORMALIZED_SPEC_SHA256, PINNED_SPEC_VERSION, TossEndpoint,
        TossHttpClient, TossSpecClient, normalized_json_sha256, pinned_spec_baseline, sha256_hex,
    };
    use domain::{DomainEvent, DomainEventKind, Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
    use serde_json::json;
    use std::collections::BTreeSet;
    use std::future::Future;
    use std::sync::Arc;
    use testkit::ScriptedHttpServer;
    use testkit_ports::spec_check::{MemoryTossSpecStatusRepository, ScriptedTossSpecSource};
    use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

    struct FixedClock(Timestamp);
    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            self.0
        }
    }

    struct EventRecorder(UnboundedSender<DomainEvent>);
    impl EventSubscriber for EventRecorder {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            self.0
                .send(event.clone())
                .map_err(|_error| SubscriberFailure::Unavailable)
        }
    }

    fn drain_events(receiver: &mut UnboundedReceiver<DomainEvent>) -> Vec<DomainEvent> {
        let mut collected = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            collected.push(event);
        }
        collected
    }

    fn stamp(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("timestamp")
    }

    fn run<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(future)
    }

    fn http_reply(status: &str, last_modified: Option<&str>, body: &str) -> Vec<u8> {
        let lm_header = match last_modified {
            Some(lm_value) => format!("Last-Modified: {lm_value}\r\n"),
            None => String::new(),
        };
        format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n{lm_header}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    fn spec_client(port: u16) -> TossSpecClient {
        let time = crate::support::time();
        let client = TossHttpClient::new(
            &TossEndpoint::local(port).expect("local"),
            Arc::<crate::support::FakeTime>::clone(&time),
            time,
        )
        .expect("client");
        TossSpecClient::new(client)
    }

    fn recorder_bus() -> (application::EventBus, UnboundedReceiver<DomainEvent>) {
        let (sender, receiver) = unbounded_channel();
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            BTreeSet::from([
                DomainEventKind::SelfCheckChanged,
                DomainEventKind::BrokerConnectionChanged,
            ]),
            EventRecorder(sender),
        );
        (builder.build(), receiver)
    }

    fn synthetic_baseline(versions_body: &str, spec_body: &str) -> TossSpecBaseline {
        TossSpecBaseline::new(
            SpecHash::try_new(broker_toss_schema::SPEC_SHA256).expect("raw"),
            normalized_json_sha256(spec_body.as_bytes()).expect("spec"),
            SpecVersionLabel::try_new("1.2.28").expect("ver"),
        )
        .with_versions_hash(normalized_json_sha256(versions_body.as_bytes()).expect("versions"))
        .with_spec_last_modified(HttpLastModified::try_new(PINNED_LAST_MODIFIED).expect("lm"))
        .with_versions_last_modified(HttpLastModified::try_new(PINNED_LAST_MODIFIED).expect("lm"))
    }

    #[test]
    fn pinned_spec_raw_and_normalized_hashes_match_repository_snapshot() {
        let raw_bytes =
            std::fs::read("../../../docs/toss-openapi-1.2.28.json").expect("pinned spec file");
        assert_eq!(
            sha256_hex(&raw_bytes),
            broker_toss_schema::SPEC_SHA256,
            "raw spec sha256 matches generated schema"
        );
        let normalized = normalized_json_sha256(&raw_bytes).expect("normalized hash");
        assert_eq!(
            normalized.as_str(),
            PINNED_NORMALIZED_SPEC_SHA256,
            "normalized spec sha256 matches pinned constant"
        );
        let baseline = pinned_spec_baseline().expect("baseline");
        assert_eq!(baseline.version_label().as_str(), PINNED_SPEC_VERSION);
        assert_eq!(
            baseline.raw_spec_hash().as_str(),
            broker_toss_schema::SPEC_SHA256
        );
        assert_eq!(
            baseline.normalized_spec_hash().as_str(),
            PINNED_NORMALIZED_SPEC_SHA256
        );
        assert_eq!(
            baseline.spec_last_modified().map(HttpLastModified::as_str),
            Some(PINNED_LAST_MODIFIED)
        );
    }

    #[test]
    fn normalized_json_hash_ignores_key_order_and_whitespace_but_catches_content_edits() {
        let left = br#"{"b": 1, "a": {"y": true, "x": [1, 2]}, "info": {"version": "1.2.28"}}"#;
        let reordered = br#"{"info":{"version":"1.2.28"},"a":{"x":[1,2],"y":true},"b":1}"#;
        let same_version_changed =
            br#"{"info":{"version":"1.2.28"},"a":{"x":[1,2,3],"y":true},"b":1}"#;
        let left_hash = normalized_json_sha256(left).expect("left");
        let right_hash = normalized_json_sha256(reordered).expect("right");
        let changed_hash = normalized_json_sha256(same_version_changed).expect("changed");
        assert_eq!(left_hash, right_hash, "canonical JSON hash must match");
        assert_ne!(
            left_hash, changed_hash,
            "same-version payload change must change normalized hash"
        );
    }

    #[test]
    fn aligned_public_docs_check_sends_no_credentials_and_records_status_quietly() {
        let versions_body =
            r#"{"current":"1.2.28","versions":["1.0","1.1","1.2"],"futureExtraField":{"ok":true}}"#;
        let spec_body =
            r#"{"openapi":"3.1.0","info":{"version":"1.2.28"},"paths":{},"x-new-meta":42}"#;
        let baseline = synthetic_baseline(versions_body, spec_body);

        let server = ScriptedHttpServer::start(vec![
            http_reply("200 OK", Some(PINNED_LAST_MODIFIED), versions_body),
            http_reply("200 OK", Some(PINNED_LAST_MODIFIED), spec_body),
        ])
        .expect("server");
        let source = spec_client(server.port());
        let repo = MemoryTossSpecStatusRepository::default();
        let clock = FixedClock(stamp(1_700_000_100));
        let (bus, mut receiver) = recorder_bus();

        let report =
            run(CheckTossSpec::new(&source, &repo, &clock, &bus).check(&baseline)).expect("check");
        assert_eq!(report.record.status(), &SpecDriftStatus::Aligned);
        assert_eq!(
            report.record.status().review_state(),
            SchemaReviewState::NotRequired
        );
        assert!(report.delivery.is_none());
        assert_eq!(drain_events(&mut receiver), Vec::new());
        assert_eq!(run(repo.latest()).expect("latest"), Some(report.record));

        let wire = String::from_utf8(server.finish().expect("finish")).expect("utf8");
        assert!(wire.contains("GET /openapi-docs/versions.json HTTP/1.1"));
        assert!(wire.contains("GET /openapi-docs/latest/openapi.json HTTP/1.1"));
        assert!(!wire.to_ascii_lowercase().contains("authorization:"));
        assert!(!wire.to_ascii_lowercase().contains("x-tossinvest-account:"));
    }

    #[test]
    fn drift_on_normalized_hash_versions_json_and_last_modified_marks_review_pending() {
        let base_versions = r#"{"current":"1.2.28","versions":["1.0","1.1","1.2"]}"#;
        let base_spec = r#"{"openapi":"3.1.0","info":{"version":"1.2.28"},"paths":{}}"#;
        let baseline = synthetic_baseline(base_versions, base_spec);
        let silent_spec_change =
            r#"{"openapi":"3.1.0","info":{"version":"1.2.28"},"paths":{"/api/v1/new":{}}}"#;
        let bumped_versions = r#"{"current":"1.2.29","versions":["1.0","1.1","1.2","1.3"]}"#;
        let newer_lm = "Thu, 05 Nov 2026 02:00:00 GMT";

        let server = ScriptedHttpServer::start(vec![
            http_reply("200 OK", Some(PINNED_LAST_MODIFIED), base_versions),
            http_reply("200 OK", Some(PINNED_LAST_MODIFIED), silent_spec_change),
            http_reply("200 OK", Some(PINNED_LAST_MODIFIED), bumped_versions),
            http_reply("200 OK", Some(PINNED_LAST_MODIFIED), base_spec),
            http_reply("200 OK", Some(PINNED_LAST_MODIFIED), base_versions),
            http_reply("200 OK", Some(newer_lm), base_spec),
        ])
        .expect("server");
        let source = spec_client(server.port());
        let repo = MemoryTossSpecStatusRepository::default();
        let clock = FixedClock(stamp(1_700_000_200));
        let (bus, mut receiver) = recorder_bus();
        let checker = CheckTossSpec::new(&source, &repo, &clock, &bus);

        for expected_reason in [
            SpecChangeReason::NormalizedHash,
            SpecChangeReason::VersionsCatalog,
            SpecChangeReason::LastModified,
        ] {
            let report = run(checker.check(&baseline)).expect("drift");
            assert_eq!(
                report.record.status(),
                &SpecDriftStatus::DriftDetected {
                    reasons: vec![expected_reason],
                    review: SchemaReviewState::PendingTypeDiffReview,
                }
            );
            assert_eq!(
                report.record.status().review_state(),
                SchemaReviewState::PendingTypeDiffReview
            );
            assert!(report.delivery.is_some());
        }
        assert_eq!(
            drain_events(&mut receiver),
            vec![
                DomainEvent::SelfCheckChanged,
                DomainEvent::SelfCheckChanged,
                DomainEvent::SelfCheckChanged,
            ]
        );
        assert_eq!(repo.records().len(), 3);
        let _wire = server.finish().expect("finish");
    }

    #[test]
    fn unchanged_drift_suppresses_duplicate_events_and_fetch_errors_record_without_banner() {
        let baseline = pinned_spec_baseline().expect("baseline");
        let source = ScriptedTossSpecSource::default();
        let drifted_obs = TossSpecObservation::new(
            SpecDocumentSnapshot::new(
                SpecHash::try_new(&"a".repeat(64)).expect("hash"),
                Some(SpecVersionLabel::try_new("1.2.28").expect("ver")),
                Some(HttpLastModified::try_new(PINNED_LAST_MODIFIED).expect("lm")),
            ),
            SpecDocumentSnapshot::new(
                SpecHash::try_new(&"b".repeat(64)).expect("vhash"),
                Some(SpecVersionLabel::try_new("1.2.28").expect("ver")),
                Some(HttpLastModified::try_new(PINNED_LAST_MODIFIED).expect("lm")),
            ),
        );
        source.script_fetch().push(Ok(drifted_obs.clone()));
        source.script_fetch().push(Ok(drifted_obs));
        source.script_fetch().push(Err(SpecFetchError::ServerError));

        let repo = MemoryTossSpecStatusRepository::default();
        let clock = FixedClock(stamp(1_700_000_300));
        let (bus, mut receiver) = recorder_bus();
        let checker = CheckTossSpec::new(&source, &repo, &clock, &bus);

        let first = run(checker.check(&baseline)).expect("first");
        assert!(first.delivery.is_some());
        let repeat = run(checker.check(&baseline)).expect("repeat");
        assert!(repeat.delivery.is_none());
        let failed = run(checker.check(&baseline)).expect("failed");
        assert_eq!(
            failed.record.status(),
            &SpecDriftStatus::CheckFailed(SpecFetchError::ServerError)
        );
        assert!(failed.delivery.is_some());
        assert_eq!(source.call_count(), 3);
        assert_eq!(drain_events(&mut receiver).len(), 2);
    }

    #[test]
    fn existing_decoders_ignore_unreviewed_new_fields_and_preserve_unknown_enums() {
        let extra_account = json!({
            "accountSeq": 101_i64,
            "accountType": "FUTURE_NEW_ACCOUNT_TYPE",
            "accountNo": "1234-5678",
            "unreviewedFutureField": {"nested": [1_i32, 2_i32, 3_i32]}
        });
        let account: broker_toss_schema::types::Account =
            serde_json::from_value(extra_account).expect("account ignores extra fields");
        assert_eq!(account.seq, 101);
        assert_eq!(account.kind.to_string(), "FUTURE_NEW_ACCOUNT_TYPE");

        let extra_price = json!({
            "symbol": "005930",
            "currency": "KRW",
            "lastPrice": "70000",
            "timestamp": "2026-03-28T15:30:00+09:00",
            "newUnreviewedField": "must-not-break"
        });
        let price: broker_toss_schema::types::PriceResponse =
            serde_json::from_value(extra_price).expect("price ignores extra fields");
        assert_eq!(price.symbol, "005930");
        assert_eq!(&*price.last_price, "70000");
    }
}
