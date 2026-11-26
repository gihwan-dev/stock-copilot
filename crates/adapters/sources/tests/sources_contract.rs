#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::future::{Future, ready};
    use std::pin::Pin;
    use std::sync::{
        Arc, RwLock,
        atomic::{AtomicU64, Ordering},
        mpsc::{Sender, channel},
    };
    use std::task::{Context, Waker};
    use std::time::Duration;

    use application::{
        AiReadPolicy, Clock, ConditionalGet, CredentialSource, EventBusBuilder, EventSubscriber,
        GetSourceStatuses, InformationError, RecordSourceStatus, ResetTime, ResponseMismatch,
        SecretValue, SourceAttemptOutcome, SourceState, SourceStatus, SourceStatusRepository,
        SubscriberFailure, SystemPortError, UserAgentRequirement, WriteCompleted,
    };
    use domain::{
        DataSource, DomainEvent, DomainEventKind, Nanosecond, OffsetSeconds, Timestamp, UnixSeconds,
    };
    use sources::{
        AiReadSettingPolicy, BROWSER_USER_AGENT, ConditionalValidators, ContactUserAgent,
        CredentialRequirement, HostAllowance, HttpDataSource, RateLimitBucket, STANDARD_USER_AGENT,
        SourceCredential, SourceEndpoint, SourceFetch, SourceFetchOutcome, SourceHttpClient,
        SourceHttpError, SourceRegistry, SourceWait, ValidatorHeader, outcome_to_attempt,
    };
    use testkit::{
        ConnectionObservation, ConnectionProbe, GateObservation, HoldPosition, ScriptedHttpServer,
    };

    #[derive(Clone, Copy)]
    enum WaitMode {
        AutoAdvance,
        Hold,
    }

    struct SteppedTime {
        nanos: AtomicU64,
        waits: RwLock<Vec<Duration>>,
        mode: WaitMode,
    }

    impl SteppedTime {
        fn new(mode: WaitMode) -> Self {
            Self {
                nanos: AtomicU64::new(0),
                waits: RwLock::new(Vec::new()),
                mode,
            }
        }

        fn set_nanos(&self, value: u64) {
            self.nanos.store(value, Ordering::SeqCst);
        }

        fn recorded_waits(&self) -> Vec<Duration> {
            self.waits.read().expect("waits").clone()
        }
    }

    impl Clock for SteppedTime {
        fn now(&self) -> Timestamp {
            let nanos = self.nanos.load(Ordering::SeqCst);
            let seconds = 1_800_000_000_i64.saturating_add(
                i64::try_from(nanos.checked_div(1_000_000_000).expect("divisor")).expect("seconds"),
            );
            Timestamp::from_parts(
                UnixSeconds::new(seconds),
                Nanosecond::new(u32::try_from(nanos % 1_000_000_000).expect("nanos")),
                OffsetSeconds::new(0),
            )
            .expect("timestamp")
        }
    }

    impl SourceWait for SteppedTime {
        fn wait(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            self.waits.write().expect("waits").push(duration);
            match self.mode {
                WaitMode::AutoAdvance => {
                    let delta = u64::try_from(duration.as_nanos()).expect("duration nanos");
                    self.nanos.fetch_add(delta, Ordering::SeqCst);
                    Box::pin(ready(()))
                }
                WaitMode::Hold => Box::pin(std::future::pending()),
            }
        }
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
    }

    fn ok_response(body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/xml\r\nETag: \"v1\"\r\nLast-Modified: Wed, 21 Oct 2026 07:28:00 GMT\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    fn not_modified_response() -> Vec<u8> {
        b"HTTP/1.1 304 Not Modified\r\nETag: \"v1\"\r\nConnection: close\r\n\r\n".to_vec()
    }

    fn matches_modified(outcome: &SourceFetchOutcome) -> bool {
        match outcome {
            SourceFetchOutcome::Modified(_) => true,
            SourceFetchOutcome::NotModified | SourceFetchOutcome::HttpFailure { .. } => false,
        }
    }

    fn assert_fetch_pending(client: &SourceHttpClient, target: &SourceFetch) {
        let mut pending = Box::pin(client.fetch(target));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(Pin::as_mut(&mut pending).poll(&mut cx).is_pending());
    }

    #[test]
    fn registry_covers_every_http_source_hosts_and_intervals() {
        assert_eq!(SourceRegistry::all().len(), 17);
        assert_eq!(HttpDataSource::ALL.len(), 17);
        assert_eq!(RateLimitBucket::ALL.len(), 15);
        assert_eq!(
            HttpDataSource::try_from_data_source(DataSource::WebSearch),
            None
        );

        let mut seen = BTreeSet::new();
        for source in HttpDataSource::ALL {
            let spec = SourceRegistry::spec(source);
            assert_eq!(spec.source(), source);
            assert!(seen.insert(spec.data_source()));
            assert_eq!(
                HttpDataSource::try_from_data_source(spec.data_source()),
                Some(source)
            );
            let empty: &[&str] = &[];
            assert_ne!(spec.allowed_hosts(), empty);
            assert_eq!(spec.min_interval(), Duration::from_secs(5));
            assert_eq!(spec.bucket().min_interval(), Duration::from_secs(5));
            for host in spec.allowed_hosts() {
                assert_eq!(spec.host_allowance(host), HostAllowance::Allowed);
                assert_eq!(
                    spec.host_allowance(&host.to_ascii_uppercase()),
                    HostAllowance::Allowed
                );
                assert_eq!(
                    spec.host_allowance(&format!("{host}.")),
                    HostAllowance::Denied
                );
                assert_eq!(
                    spec.host_allowance(&format!("evil.{host}")),
                    HostAllowance::Denied
                );
                assert_eq!(
                    spec.host_allowance(&format!("{host}.evil.com")),
                    HostAllowance::Denied
                );
            }
        }
    }

    #[test]
    fn registry_conditional_get_support_matches_documented_sources() {
        for supported in [
            HttpDataSource::Hankyung,
            HttpDataSource::Edaily,
            HttpDataSource::FederalReserve,
        ] {
            assert_eq!(
                SourceRegistry::spec(supported).conditional_get(),
                ConditionalGet::Supported
            );
        }
        for unsupported in [
            HttpDataSource::KindDisclosure,
            HttpDataSource::DartDisclosureApi,
            HttpDataSource::DartDisclosureRss,
            HttpDataSource::DartFundamentals,
            HttpDataSource::KindCorpList,
            HttpDataSource::Maekyung,
            HttpDataSource::Yonhap,
            HttpDataSource::GoogleNews,
            HttpDataSource::YahooFinance,
            HttpDataSource::SecEdgar,
            HttpDataSource::BankOfKorea,
            HttpDataSource::UsTreasury,
            HttpDataSource::Fred,
        ] {
            assert_eq!(
                SourceRegistry::spec(unsupported).conditional_get(),
                ConditionalGet::Unsupported
            );
        }
    }

    #[test]
    fn registry_policies_match_headers_credentials_and_host_buckets() {
        assert_eq!(
            SourceRegistry::spec(HttpDataSource::YahooFinance).user_agent(),
            UserAgentRequirement::Browser
        );
        assert_eq!(
            SourceRegistry::spec(HttpDataSource::SecEdgar).user_agent(),
            UserAgentRequirement::Contact
        );
        assert_eq!(
            SourceRegistry::spec(HttpDataSource::SecEdgar).max_calls_per_second(),
            Some(10)
        );
        assert_eq!(
            SourceRegistry::spec(HttpDataSource::Yonhap).ai_read_policy(),
            AiReadSettingPolicy::LockedOff
        );
        assert_eq!(
            SourceRegistry::spec(HttpDataSource::GoogleNews).default_ai_read(),
            AiReadPolicy::TitleOnly
        );
        assert_eq!(
            SourceRegistry::spec(HttpDataSource::DartDisclosureApi).credential(),
            CredentialRequirement::Optional(CredentialSource::Dart)
        );
        assert_eq!(
            SourceRegistry::spec(HttpDataSource::DartFundamentals).credential(),
            CredentialRequirement::Optional(CredentialSource::Dart)
        );
        assert_eq!(
            SourceRegistry::spec(HttpDataSource::Fred).credential(),
            CredentialRequirement::Optional(CredentialSource::Fred)
        );
        assert_eq!(
            SourceRegistry::spec(HttpDataSource::KindDisclosure).bucket(),
            SourceRegistry::spec(HttpDataSource::KindCorpList).bucket()
        );
        assert_eq!(
            SourceRegistry::spec(HttpDataSource::DartDisclosureApi).bucket(),
            SourceRegistry::spec(HttpDataSource::DartFundamentals).bucket()
        );
    }

    #[test]
    fn sec_edgar_without_contact_makes_zero_connections_and_validates_contact_header() {
        for bad in [
            "",
            "   ",
            "onlyoneword",
            "no-email-here app",
            "AppName bad@localhost",
            "AppName user@a.1",
            "AppName a@b.com c@d.com",
            "AppName user@example.com\r\nX-Injected: 1",
            "AppName user@example.com\nInjected",
            "AppName user@example.com\r\n",
            "AppName user@example.com\t",
            "이름 user@example.com",
            "12345 user@example.com",
        ] {
            assert_eq!(
                ContactUserAgent::try_new(bad).expect_err("invalid contact"),
                SourceHttpError::InvalidHeader,
                "{bad}"
            );
        }
        for bad_validator in ["", "   ", "\"v1\"\r\n", "\"v1\"\t", "\"v1\"\nX: 1"] {
            assert_eq!(
                ValidatorHeader::try_new(bad_validator).expect_err("invalid validator"),
                SourceHttpError::InvalidHeader,
                "{bad_validator}"
            );
        }

        let valid =
            ContactUserAgent::try_new("StockCopilot tester@example.com").expect("valid contact");
        assert_eq!(format!("{valid:?}"), "ContactUserAgent(<redacted>)");

        let probe = ConnectionProbe::bind().expect("probe");
        let time = Arc::new(SteppedTime::new(WaitMode::AutoAdvance));
        let client = SourceHttpClient::new(
            SourceEndpoint::local(probe.port().expect("port")).expect("endpoint"),
            Arc::<SteppedTime>::clone(&time),
            time,
            None,
        )
        .expect("client");

        let target = SourceFetch::new(
            HttpDataSource::SecEdgar,
            "https://data.sec.gov/submissions/CIK0000320193.json",
        );
        assert_eq!(
            runtime().block_on(client.fetch(&target)),
            Err(SourceHttpError::MissingContactUserAgent)
        );
        assert_eq!(
            probe.observe().expect("probe state"),
            ConnectionObservation::NoConnection
        );
    }

    fn local_client(
        port: u16,
        time: Arc<SteppedTime>,
        contact: Option<ContactUserAgent>,
    ) -> SourceHttpClient {
        SourceHttpClient::new(
            SourceEndpoint::local(port).expect("endpoint"),
            Arc::<SteppedTime>::clone(&time),
            time,
            contact,
        )
        .expect("client")
    }

    fn sample_validators() -> ConditionalValidators {
        ConditionalValidators::new(
            Some(ValidatorHeader::try_new("\"v1\"").expect("etag")),
            Some(ValidatorHeader::try_new("Wed, 21 Oct 2026 07:28:00 GMT").expect("modified")),
        )
    }

    fn assert_hankyung_payload(outcome: &SourceFetchOutcome) {
        let SourceFetchOutcome::Modified(payload) = outcome else {
            panic!("expected modified");
        };
        assert_eq!(payload.status(), 200);
        assert_eq!(payload.body(), b"<rss>hankyung</rss>");
        assert_eq!(payload.etag(), Some("\"v1\""));
        assert_eq!(
            payload.last_modified(),
            Some("Wed, 21 Oct 2026 07:28:00 GMT")
        );
        assert_eq!(payload.content_type(), Some("application/xml"));
        assert!(!format!("{payload:?}").contains("<rss>hankyung</rss>"));
    }

    #[test]
    fn conditional_get_is_sent_only_for_supported_sources() {
        let server = ScriptedHttpServer::start(vec![
            ok_response("<rss>hankyung</rss>"),
            not_modified_response(),
            ok_response("<rss>maekyung</rss>"),
        ])
        .expect("server");
        let time = Arc::new(SteppedTime::new(WaitMode::AutoAdvance));
        let client = local_client(server.port(), Arc::<SteppedTime>::clone(&time), None);
        let validators = sample_validators();
        let rt = runtime();
        let hankyung = SourceFetch::new(
            HttpDataSource::Hankyung,
            "https://www.hankyung.com/feed/finance",
        )
        .with_validators(validators.clone());
        assert_hankyung_payload(&rt.block_on(client.fetch(&hankyung)).expect("first"));
        assert_eq!(
            rt.block_on(client.fetch(&hankyung)).expect("second"),
            SourceFetchOutcome::NotModified
        );
        assert_eq!(time.recorded_waits(), vec![Duration::from_secs(5)]);
        let maekyung = SourceFetch::new(
            HttpDataSource::Maekyung,
            "https://www.mk.co.kr/rss/50200011/",
        )
        .with_validators(validators);
        assert!(matches_modified(
            &rt.block_on(client.fetch(&maekyung)).expect("third")
        ));
        let wire = String::from_utf8(server.finish().expect("finish")).expect("utf8");
        let sections: Vec<&str> = wire.split("GET ").skip(1).collect();
        assert_eq!(sections.len(), 3);
        assert!(sections[0].contains("if-none-match: \"v1\""));
        assert!(sections[0].contains("if-modified-since: Wed, 21 Oct 2026 07:28:00 GMT"));
        assert!(!sections[2].to_ascii_lowercase().contains("if-none-match"));
        assert!(
            !sections[2]
                .to_ascii_lowercase()
                .contains("if-modified-since")
        );
    }

    #[test]
    fn per_source_user_agents_match_standard_browser_and_sec_contact() {
        let server = ScriptedHttpServer::start(vec![
            ok_response("<rss>maekyung</rss>"),
            ok_response("<rss>yahoo</rss>"),
            ok_response("{}"),
        ])
        .expect("server");
        let time = Arc::new(SteppedTime::new(WaitMode::AutoAdvance));
        let contact =
            ContactUserAgent::try_new("StockCopilot sec-contact@example.com").expect("contact");
        let client = SourceHttpClient::new(
            SourceEndpoint::local(server.port()).expect("endpoint"),
            Arc::<SteppedTime>::clone(&time),
            time,
            Some(contact),
        )
        .expect("client");

        let rt = runtime();
        for (source, url) in [
            (
                HttpDataSource::Maekyung,
                "https://www.mk.co.kr/rss/50200011/",
            ),
            (
                HttpDataSource::YahooFinance,
                "https://feeds.finance.yahoo.com/rss/2.0/headline?s=AAPL",
            ),
            (
                HttpDataSource::SecEdgar,
                "https://data.sec.gov/submissions/CIK0000320193.json",
            ),
        ] {
            let outcome = rt
                .block_on(client.fetch(&SourceFetch::new(source, url)))
                .expect("fetch");
            assert!(matches_modified(&outcome));
        }

        let wire = String::from_utf8(server.finish().expect("finish")).expect("utf8");
        let sections: Vec<&str> = wire.split("GET ").skip(1).collect();
        assert_eq!(sections.len(), 3);
        assert!(sections[0].contains(&format!("user-agent: {STANDARD_USER_AGENT}")));
        assert!(sections[1].contains(&format!("user-agent: {BROWSER_USER_AGENT}")));
        assert!(sections[2].contains("user-agent: StockCopilot sec-contact@example.com"));
    }

    #[test]
    fn disallowed_urls_make_zero_connections() {
        let probe = ConnectionProbe::bind().expect("probe");
        let time = Arc::new(SteppedTime::new(WaitMode::AutoAdvance));
        let client = SourceHttpClient::new(
            SourceEndpoint::local(probe.port().expect("port")).expect("endpoint"),
            Arc::<SteppedTime>::clone(&time),
            time,
            None,
        )
        .expect("client");

        let rt = runtime();
        for bad_url in [
            "http://www.hankyung.com/feed/finance",
            "https://www.mk.co.kr/rss/50200011/",
            "https://evil.com/feed/finance",
            "https://www.hankyung.com.evil.com/feed",
            "https://sub.www.hankyung.com/feed",
            "https://www.hankyung.com./feed",
            "https://www.hankyung.com:443/feed",
            "https://www.hankyung.com:8443/feed",
            "https://user:pass@www.hankyung.com/feed",
            "https://user@www.hankyung.com/feed",
            "https://127.0.0.1/feed",
            "https://www.hankyung.com/../secret",
            "https://www.hankyung.com/%2e%2e/secret",
            "https://www.hankyung.com/%252e/secret",
            "https://www.hankyung.com/feed#frag",
            "https://www.hankyung.com/feed#",
            "https://www.hankyung.com/feed?a=1%0d%0aX:1",
            "https://www.hankyung.com/feed\\evil",
            "https://www.hankyung.com/feed\r\n",
            "https://www.hankyung.com/feed?api_key=smuggled",
            "https://www.hankyung.com/feed?crtfc_key=smuggled",
            "https://www.hankyung.com/feed?%61pi_key=smuggled",
            "https://www.hankyung.com/feed?ok=1;crtfc_key=smuggled",
            "https://www.hankyung.com/feed?ok=1;api%5fkey=smuggled",
            "https://www.hankyung.com/feed?ok=1;%63rtfc_key=smuggled",
            "https://www.hankyung.com/feed?ok=1;%61pi%5fkey=smuggled",
        ] {
            assert_eq!(
                rt.block_on(client.fetch(&SourceFetch::new(HttpDataSource::Hankyung, bad_url))),
                Err(SourceHttpError::DisallowedUrl),
                "{bad_url}"
            );
        }
        assert_eq!(
            probe.observe().expect("probe"),
            ConnectionObservation::NoConnection
        );
    }

    #[test]
    fn redirects_and_unexpected_not_modified_are_rejected() {
        let redirect_server = ScriptedHttpServer::start(vec![
            b"HTTP/1.1 302 Found\r\nLocation: https://evil.example/leak\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
            not_modified_response(),
        ])
        .expect("redirect server");
        let time = Arc::new(SteppedTime::new(WaitMode::AutoAdvance));
        let client = SourceHttpClient::new(
            SourceEndpoint::local(redirect_server.port()).expect("endpoint"),
            Arc::<SteppedTime>::clone(&time),
            time,
            None,
        )
        .expect("client");

        let rt = runtime();
        assert_eq!(
            rt.block_on(client.fetch(&SourceFetch::new(
                HttpDataSource::Hankyung,
                "https://www.hankyung.com/feed/finance",
            ))),
            Err(SourceHttpError::RedirectBlocked)
        );
        assert_eq!(
            rt.block_on(client.fetch(&SourceFetch::new(
                HttpDataSource::Maekyung,
                "https://www.mk.co.kr/rss/50200011/",
            ))),
            Err(SourceHttpError::UnexpectedNotModified)
        );
        let _wire = redirect_server.finish().expect("finish");
    }

    async fn wait_for_gate(gate: testkit::ResponseGate) -> testkit::ResponseGate {
        for _ in 0_u32..200_u32 {
            if gate.observe() == GateObservation::Held {
                return gate;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("first call must reach server and hold");
    }

    fn spawn_fetch(
        client: &Arc<SourceHttpClient>,
        source: HttpDataSource,
        url: &'static str,
    ) -> tokio::task::JoinHandle<Result<SourceFetchOutcome, SourceHttpError>> {
        let cloned = Arc::clone(client);
        tokio::spawn(async move { cloned.fetch(&SourceFetch::new(source, url)).await })
    }

    #[test]
    fn per_source_concurrency_is_one_and_other_sources_proceed_independently() {
        let (same_server, gate) = ScriptedHttpServer::start_holding(
            vec![ok_response("first"), ok_response("second")],
            HoldPosition::new(0),
        )
        .expect("holding server");
        let other_server = ScriptedHttpServer::start(vec![ok_response("other")]).expect("other");
        let time = Arc::new(SteppedTime::new(WaitMode::AutoAdvance));
        let client = Arc::new(local_client(
            same_server.port(),
            Arc::<SteppedTime>::clone(&time),
            None,
        ));
        let other_client = local_client(other_server.port(), time, None);
        runtime().block_on(async {
            let first_task = spawn_fetch(
                &client,
                HttpDataSource::KindDisclosure,
                "https://kind.krx.co.kr/disclosure/rsstodaydistribute.do?currentPageSize=100",
            );
            let held_gate = wait_for_gate(gate).await;
            let second_task = spawn_fetch(
                &client,
                HttpDataSource::KindCorpList,
                "https://kind.krx.co.kr/corpgeneral/corpList.do?searchType=13",
            );
            tokio::time::sleep(Duration::from_millis(30)).await;
            assert!(!second_task.is_finished(), "same host bucket must wait");
            let independent = other_client
                .fetch(&SourceFetch::new(
                    HttpDataSource::FederalReserve,
                    "https://www.federalreserve.gov/feeds/press_all.xml",
                ))
                .await
                .expect("independent source");
            assert!(matches_modified(&independent));
            held_gate.release().expect("release");
            assert!(matches_modified(
                &first_task.await.expect("join first").expect("first ok")
            ));
            assert!(matches_modified(
                &second_task.await.expect("join second").expect("second ok")
            ));
        });
        let _same_wire = same_server.finish().expect("same finish");
        let _other_wire = other_server.finish().expect("other finish");
    }

    #[test]
    fn minimum_five_second_interval_boundary_clock_rollback_and_retry_after() {
        let server = ScriptedHttpServer::start(vec![
            ok_response("one"),
            ok_response("two"),
            b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 12\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
        ])
        .expect("server");
        let time = Arc::new(SteppedTime::new(WaitMode::Hold));
        let client = SourceHttpClient::new(
            SourceEndpoint::local(server.port()).expect("endpoint"),
            Arc::<SteppedTime>::clone(&time),
            Arc::<SteppedTime>::clone(&time),
            None,
        )
        .expect("client");

        let rt = runtime();
        let target = SourceFetch::new(
            HttpDataSource::Edaily,
            "https://rss.edaily.co.kr/stock_news.xml",
        );
        assert!(matches_modified(
            &rt.block_on(client.fetch(&target)).expect("first")
        ));

        time.set_nanos(4_999_999_999);
        assert_fetch_pending(&client, &target);
        assert_eq!(time.recorded_waits(), vec![Duration::from_nanos(1)]);

        time.set_nanos(1);
        assert_fetch_pending(&client, &target);
        assert_eq!(
            time.recorded_waits().last().copied(),
            Some(Duration::from_nanos(4_999_999_999))
        );

        time.set_nanos(5_000_000_000);
        assert!(matches_modified(
            &rt.block_on(client.fetch(&target)).expect("second")
        ));

        time.set_nanos(10_000_000_000);
        assert_eq!(
            rt.block_on(client.fetch(&target)).expect("429"),
            SourceFetchOutcome::HttpFailure {
                status: 429,
                retry_after_seconds: Some(12),
            }
        );

        time.set_nanos(21_999_999_999);
        assert_fetch_pending(&client, &target);
        assert_eq!(
            time.recorded_waits().last().copied(),
            Some(Duration::from_nanos(1))
        );
        let _finished = server.finish().expect("finish");
    }

    #[test]
    fn service_unavailable_retry_after_also_defers_bucket() {
        let server = ScriptedHttpServer::start(vec![
            b"HTTP/1.1 503 Service Unavailable\r\nRetry-After: 9\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
        ])
        .expect("server");
        let time = Arc::new(SteppedTime::new(WaitMode::Hold));
        let client = SourceHttpClient::new(
            SourceEndpoint::local(server.port()).expect("endpoint"),
            Arc::<SteppedTime>::clone(&time),
            Arc::<SteppedTime>::clone(&time),
            None,
        )
        .expect("client");
        let target = SourceFetch::new(
            HttpDataSource::BankOfKorea,
            "https://www.bok.or.kr/portal/bbs/B0000245/list.do",
        );
        let rt = runtime();
        assert_eq!(
            rt.block_on(client.fetch(&target)).expect("503"),
            SourceFetchOutcome::HttpFailure {
                status: 503,
                retry_after_seconds: Some(9),
            }
        );
        time.set_nanos(8_999_999_999);
        assert_fetch_pending(&client, &target);
        assert_eq!(time.recorded_waits(), vec![Duration::from_nanos(1)]);
        let _finished = server.finish().expect("finish");
    }

    #[test]
    fn registry_hosts_and_credentials_exclude_broker_credentials() {
        let allowed: BTreeSet<&str> = SourceRegistry::all()
            .iter()
            .flat_map(|spec| spec.allowed_hosts().iter().copied())
            .collect();
        let expected: BTreeSet<&str> = BTreeSet::from([
            "api.stlouisfed.org",
            "dart.fss.or.kr",
            "data.sec.gov",
            "feeds.finance.yahoo.com",
            "finance.yahoo.com",
            "home.treasury.gov",
            "kind.krx.co.kr",
            "news.google.com",
            "opendart.fss.or.kr",
            "rss.edaily.co.kr",
            "www.bok.or.kr",
            "www.edaily.co.kr",
            "www.federalreserve.gov",
            "www.hankyung.com",
            "www.mk.co.kr",
            "www.sec.gov",
            "www.segyefn.com",
            "www.yna.co.kr",
        ]);
        assert_eq!(allowed, expected);
        for spec in SourceRegistry::all() {
            match spec.credential() {
                CredentialRequirement::None
                | CredentialRequirement::Optional(
                    CredentialSource::Dart | CredentialSource::Fred,
                ) => {}
            }
        }
    }

    #[test]
    fn credentials_reject_mismatched_and_injected_values() {
        let probe = ConnectionProbe::bind().expect("probe");
        let time = Arc::new(SteppedTime::new(WaitMode::AutoAdvance));
        let client = SourceHttpClient::new(
            SourceEndpoint::local(probe.port().expect("port")).expect("endpoint"),
            Arc::<SteppedTime>::clone(&time),
            time,
            None,
        )
        .expect("client");
        assert_eq!(format!("{client:?}"), "SourceHttpClient(<redacted>)");

        let rt = runtime();
        for (source, url, cred_source, bytes, expected) in [
            (
                HttpDataSource::Hankyung,
                "https://www.hankyung.com/feed/finance",
                CredentialSource::Dart,
                b"fake-key".as_slice(),
                SourceHttpError::InvalidCredential,
            ),
            (
                HttpDataSource::Fred,
                "https://api.stlouisfed.org/fred/series/observations?series_id=DGS10",
                CredentialSource::Dart,
                b"fake-key".as_slice(),
                SourceHttpError::InvalidCredential,
            ),
            (
                HttpDataSource::DartDisclosureApi,
                "https://opendart.fss.or.kr/api/list.json?bgn_de=20261101",
                CredentialSource::Dart,
                b"bad&injected=1".as_slice(),
                SourceHttpError::InvalidCredential,
            ),
            (
                HttpDataSource::DartDisclosureApi,
                "https://opendart.fss.or.kr/api/list.json?crtfc_key=dup",
                CredentialSource::Dart,
                b"fake-dart-key-991".as_slice(),
                SourceHttpError::DisallowedUrl,
            ),
        ] {
            let fetch = SourceFetch::new(source, url).with_credential(SourceCredential::new(
                cred_source,
                SecretValue::new(bytes.to_vec()),
            ));
            assert_eq!(rt.block_on(client.fetch(&fetch)), Err(expected));
        }
        assert_eq!(
            probe.observe().expect("probe"),
            ConnectionObservation::NoConnection
        );
    }

    #[test]
    fn valid_dart_and_fred_credentials_are_attached_and_redacted_in_debug() {
        let server =
            ScriptedHttpServer::start(vec![ok_response("{}"), ok_response("{}")]).expect("server");
        let time = Arc::new(SteppedTime::new(WaitMode::AutoAdvance));
        let client = SourceHttpClient::new(
            SourceEndpoint::local(server.port()).expect("endpoint"),
            Arc::<SteppedTime>::clone(&time),
            time,
            None,
        )
        .expect("client");

        let dart_cred = SourceCredential::new(
            CredentialSource::Dart,
            SecretValue::new(b"fake-dart-key-991".to_vec()),
        );
        assert!(!format!("{dart_cred:?}").contains("fake-dart-key-991"));
        let dart_fetch = SourceFetch::new(
            HttpDataSource::DartDisclosureApi,
            "https://opendart.fss.or.kr/api/list.json?bgn_de=20261101",
        )
        .with_credential(dart_cred);
        assert!(!format!("{dart_fetch:?}").contains("fake-dart-key-991"));

        let fred_fetch = SourceFetch::new(
            HttpDataSource::Fred,
            "https://api.stlouisfed.org/fred/series/observations?series_id=DGS10",
        )
        .with_credential(SourceCredential::new(
            CredentialSource::Fred,
            SecretValue::new(b"fake-fred-key-442".to_vec()),
        ));

        let rt = runtime();
        assert!(matches_modified(
            &rt.block_on(client.fetch(&dart_fetch)).expect("dart")
        ));
        assert!(matches_modified(
            &rt.block_on(client.fetch(&fred_fetch)).expect("fred")
        ));
        let wire = String::from_utf8(server.finish().expect("finish")).expect("utf8");
        assert!(wire.contains("crtfc_key=fake-dart-key-991"));
        assert!(wire.contains("api_key=fake-fred-key-442"));
    }

    #[test]
    fn response_size_limit_rejects_oversized_payloads_and_port_zero_is_invalid() {
        assert_eq!(
            SourceEndpoint::local(0).expect_err("port zero"),
            SourceHttpError::InvalidEndpoint
        );
        let _live = SourceEndpoint::live();

        let oversized_len = 8_388_609_usize;
        let mut response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {oversized_len}\r\nConnection: close\r\n\r\n"
        )
        .into_bytes();
        response.extend(vec![b'a'; oversized_len]);
        let server = ScriptedHttpServer::start(vec![response]).expect("oversized server");
        let time = Arc::new(SteppedTime::new(WaitMode::AutoAdvance));
        let client = SourceHttpClient::new(
            SourceEndpoint::local(server.port()).expect("endpoint"),
            Arc::<SteppedTime>::clone(&time),
            time,
            None,
        )
        .expect("client");

        let err = runtime()
            .block_on(client.fetch(&SourceFetch::new(
                HttpDataSource::UsTreasury,
                "https://home.treasury.gov/resource-center/data-chart-center/interest-rates/pages/xml",
            )))
            .expect_err("too large");
        assert_eq!(err, SourceHttpError::ResponseTooLarge);
        match server.finish() {
            Ok(_) | Err(_) => {}
        }
    }

    struct MemoryStatusRepo(RwLock<BTreeMap<DataSource, SourceStatus>>);
    impl SourceStatusRepository for MemoryStatusRepo {
        fn find(
            &self,
            source: DataSource,
        ) -> impl Future<Output = Result<Option<SourceStatus>, SystemPortError>> + Send {
            ready(Ok(self.0.read().expect("read").get(&source).copied()))
        }
        fn list_all(
            &self,
        ) -> impl Future<Output = Result<Vec<SourceStatus>, SystemPortError>> + Send {
            ready(Ok(self.0.read().expect("read").values().copied().collect()))
        }
        fn save(
            &self,
            status: &SourceStatus,
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            self.0
                .write()
                .expect("write")
                .insert(status.source(), *status);
            ready(Ok(WriteCompleted))
        }
    }

    struct EventSink(Sender<DomainEvent>);
    impl EventSubscriber for EventSink {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            match self.0.send(event.clone()) {
                Ok(()) => Ok(()),
                Err(_) => Err(SubscriberFailure::Unavailable),
            }
        }
    }

    #[test]
    fn http_outcome_updates_source_status_and_emits_source_status_changed() {
        let server = ScriptedHttpServer::start(vec![ok_response("<rss/>")]).expect("server");
        let time = Arc::new(SteppedTime::new(WaitMode::AutoAdvance));
        let client = SourceHttpClient::new(
            SourceEndpoint::local(server.port()).expect("endpoint"),
            Arc::<SteppedTime>::clone(&time),
            Arc::<SteppedTime>::clone(&time),
            None,
        )
        .expect("client");

        let repo = MemoryStatusRepo(RwLock::new(BTreeMap::new()));
        let (tx, rx) = channel();
        let mut builder = EventBusBuilder::default();
        let _id = builder.subscribe(
            BTreeSet::from([DomainEventKind::SourceStatusChanged]),
            EventSink(tx),
        );
        let bus = builder.build();
        let recorder = RecordSourceStatus::new(&repo, time.as_ref(), &bus);
        let reader = GetSourceStatuses::new(&repo);

        let rt = runtime();
        let ok_res = rt.block_on(client.fetch(&SourceFetch::new(
            HttpDataSource::Hankyung,
            "https://www.hankyung.com/feed/finance",
        )));
        let report = rt
            .block_on(recorder.record(
                HttpDataSource::Hankyung.data_source(),
                outcome_to_attempt(HttpDataSource::Hankyung, &ok_res),
            ))
            .expect("record");
        assert_eq!(report.status().state(), SourceState::Healthy);

        let sec_res = rt.block_on(client.fetch(&SourceFetch::new(
            HttpDataSource::SecEdgar,
            "https://data.sec.gov/submissions/CIK0000320193.json",
        )));
        let _sec_report = rt
            .block_on(recorder.record(
                HttpDataSource::SecEdgar.data_source(),
                outcome_to_attempt(HttpDataSource::SecEdgar, &sec_res),
            ))
            .expect("sec record");

        assert_eq!(rt.block_on(reader.list()).expect("list").len(), 18);
        assert_eq!(
            rx.try_iter().collect::<Vec<_>>(),
            vec![
                DomainEvent::SourceStatusChanged(DataSource::Hankyung),
                DomainEvent::SourceStatusChanged(DataSource::SecEdgar),
            ]
        );
        let _finished = server.finish().expect("finish");
    }

    #[test]
    fn outcome_to_attempt_classifies_credentials_outages_and_protocol_errors() {
        assert_eq!(
            outcome_to_attempt(
                HttpDataSource::DartDisclosureApi,
                &Ok(SourceFetchOutcome::HttpFailure {
                    status: 403,
                    retry_after_seconds: None,
                })
            ),
            SourceAttemptOutcome::Failed(InformationError::InvalidCredential(
                CredentialSource::Dart
            ))
        );
        assert_eq!(
            outcome_to_attempt(
                HttpDataSource::Hankyung,
                &Ok(SourceFetchOutcome::HttpFailure {
                    status: 403,
                    retry_after_seconds: None,
                })
            ),
            SourceAttemptOutcome::Failed(InformationError::Unavailable(ResetTime::NotProvided))
        );
        assert_eq!(
            outcome_to_attempt(
                HttpDataSource::Hankyung,
                &Err(SourceHttpError::RedirectBlocked)
            ),
            SourceAttemptOutcome::Failed(InformationError::InvalidResponse(
                ResponseMismatch::Schema
            ))
        );
        assert_eq!(
            outcome_to_attempt(
                HttpDataSource::SecEdgar,
                &Err(SourceHttpError::MissingContactUserAgent)
            ),
            SourceAttemptOutcome::NotConfigured
        );
    }

    #[test]
    fn proxy_environment_is_ignored_in_child_probe() {
        let executable = testkit::test_executable().expect("test executable");
        let output =
            testkit::proxy_probe(&executable, "tests::proxy_env_child_fetch").expect("proxy probe");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(output.status.success(), "{stdout}");
        assert!(stdout.contains("1 passed; 0 failed"), "{stdout}");
    }

    #[test]
    fn proxy_env_child_fetch() {
        let server = ScriptedHttpServer::start(vec![ok_response("direct")]).expect("server");
        let time = Arc::new(SteppedTime::new(WaitMode::AutoAdvance));
        let client = SourceHttpClient::new(
            SourceEndpoint::local(server.port()).expect("endpoint"),
            Arc::<SteppedTime>::clone(&time),
            time,
            None,
        )
        .expect("client");
        let outcome = runtime()
            .block_on(client.fetch(&SourceFetch::new(
                HttpDataSource::Hankyung,
                "https://www.hankyung.com/feed/finance",
            )))
            .expect("direct fetch");
        assert!(matches_modified(&outcome));
        let _finished = server.finish().expect("finish");
    }
}
