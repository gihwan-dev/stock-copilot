#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs::{self, OpenOptions, Permissions};
    use std::io::{self, Cursor, Read, Write};
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};
    use std::time::Duration;

    use application::Clock;
    use broker_toss::{RequestWait, TossEndpoint};
    use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
    use serde_json::Value;
    use testkit::{ConnectionObservation, ConnectionProbe, ScriptedHttpServer};
    use toss_smoke::cli::{CliError, Command, parse_args, usage_text};
    use toss_smoke::exit_code::SmokeExitCode;
    use toss_smoke::input::{
        AppStopConfirmation, CONFIRM_APP_STOPPED_PHRASE, InputError, StdinKind, no_echo_guide,
        read_input,
    };
    use toss_smoke::lock::{
        APP_DATA_DIR_NAME, DESKTOP_RUNNING_CONTENT, DESKTOP_STOPPED_CONTENT, LOCK_FILE_NAME,
        LockError, LockMode, SMOKE_RUNNING_CONTENT, SMOKE_STOPPED_CONTENT, acquire_token_lock,
    };
    use toss_smoke::record::{RecordError, detect_repo_roots, prepare_recording};
    use toss_smoke::{RuntimeContext, execute};

    const EXPECTED_NO_ECHO_COMMAND: &str = "( trap 'unset id sec confirm' EXIT; printf 'Client ID: ' >&2 && IFS= read -rs id && printf '\\nClient Secret: ' >&2 && IFS= read -rs sec && printf '\\n앱 종료 확인(앱을 껐습니다): ' >&2 && IFS= read -r confirm && printf '%s\\n%s\\n%s\\n' \"$id\" \"$sec\" \"$confirm\" | cargo run --locked -p toss-smoke -- check )";

    static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new(label: &str) -> Self {
            let seq = TEMP_SEQ.fetch_add(1, Ordering::SeqCst);
            let name = format!("toss-smoke-test-{}-{}-{seq}", std::process::id(), label);
            let path = Path::new("/tmp").join(name);
            fs::create_dir_all(&path).expect("make scratch dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            match fs::remove_dir_all(&self.path) {
                Ok(()) | Err(_) => {}
            }
        }
    }

    #[derive(Default)]
    struct FixedClock(AtomicI64);

    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            let nanos = self.0.load(Ordering::SeqCst);
            let secs = nanos.checked_div(1_000_000_000).expect("divisor");
            let sub = u32::try_from(nanos.rem_euclid(1_000_000_000)).expect("nanos");
            Timestamp::from_parts(
                UnixSeconds::new(1_800_000_000_i64.saturating_add(secs)),
                Nanosecond::new(sub),
                OffsetSeconds::new(0),
            )
            .expect("timestamp")
        }
    }

    impl RequestWait for FixedClock {
        fn wait(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            Box::pin(async move {
                let delta = i64::try_from(duration.as_nanos()).expect("duration");
                self.0.fetch_add(delta, Ordering::SeqCst);
            })
        }
    }

    fn fake_time() -> (Arc<dyn Clock>, Arc<dyn RequestWait>) {
        let shared = Arc::new(FixedClock::default());
        let cloned = Arc::clone(&shared);
        let clock: Arc<dyn Clock> = cloned;
        let wait: Arc<dyn RequestWait> = shared;
        (clock, wait)
    }

    struct SpyReader {
        reads: Arc<AtomicUsize>,
    }

    impl SpyReader {
        fn new() -> (Self, Arc<AtomicUsize>) {
            let reads = Arc::new(AtomicUsize::new(0));
            (
                Self {
                    reads: Arc::clone(&reads),
                },
                reads,
            )
        }
    }

    impl Read for SpyReader {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            Err(io::Error::other("unexpected stdin read"))
        }
    }

    fn os_args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    fn http_response(status: u16, reason: &str, body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nX-RateLimit-Limit: 20\r\nX-RateLimit-Remaining: 19\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    fn run_async<F: Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(future)
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed stdout"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed stdout"))
        }
    }

    fn happy_responses(sentinels: &Sentinels) -> Vec<Vec<u8>> {
        let token = format!(
            r#"{{"access_token":"{}","token_type":"Bearer","expires_in":86400}}"#,
            sentinels.access_token
        );
        let accounts = format!(
            r#"{{"result":[{{"accountNo":"{}","accountSeq":998877,"accountType":"OVERSEAS_DERIVATIVES"}},{{"accountNo":"{}","accountSeq":{},"accountType":"BROKERAGE"}}]}}"#,
            "99900011122", sentinels.account_no, sentinels.account_seq
        );
        let holdings = format!(
            r#"{{"result":{{"totalPurchaseAmount":{{"krw":"{}","usd":"{}"}},"marketValue":{{"amount":{{"krw":"{}","usd":"4321.11"}},"amountAfterCost":{{"krw":"7050000","usd":"4300.00"}}}},"profitLoss":{{"amount":{{"krw":"700000","usd":"232"}},"amountAfterCost":{{"krw":"550000","usd":"218.43"}},"rate":"0.1179","rateAfterCost":"0.0983"}},"dailyProfitLoss":{{"amount":{{"krw":"100000","usd":"25"}},"rate":"0.0141"}},"items":[{{"symbol":"{}","name":"{}","marketCountry":"KR","currency":"KRW","quantity":"{}","lastPrice":"{}","averagePurchasePrice":"65432","marketValue":{{"purchaseAmount":"6500000","amount":"7200000","amountAfterCost":"7050000"}},"profitLoss":{{"amount":"700000","amountAfterCost":"550000","rate":"0.1077","rateAfterCost":"0.0846"}},"dailyProfitLoss":{{"amount":"100000","rate":"0.0141"}},"cost":{{"commission":"14400","tax":"135600"}}}}]}}}}"#,
            sentinels.amount_krw,
            sentinels.amount_usd,
            sentinels.market_value_krw,
            sentinels.holding_symbol,
            sentinels.holding_name,
            sentinels.quantity,
            sentinels.last_price
        );
        let bp_krw = format!(
            r#"{{"result":{{"currency":"KRW","cashBuyingPower":"{}"}}}}"#,
            sentinels.cash_krw
        );
        let bp_usd = r#"{"result":{"currency":"USD","cashBuyingPower":"2468.13"}}"#;
        let transactions = format!(
            r#"{{"result":{{"transactions":[{{"transactionType":"BUY","marketCountry":"KR","currency":"KRW","symbol":"{}","assetClass":"STOCK","isinCode":"KR7005930003","quantity":"10","filledPrice":"{}","amount":"711110","commission":"1420","tax":"0","netAmount":"712530","exchangeRate":null,"settlementDate":"2026-09-30","settled":true,"transactionTime":"2026-09-30T09:00:00+09:00","isCancellation":false}}],"nextCursor":null,"hasNext":false}}}}"#,
            sentinels.holding_symbol, sentinels.filled_price
        );
        let fx = r#"{"result":{"baseCurrency":"USD","quoteCurrency":"KRW","rate":"1381.7","midRate":"1374.2","basisPoint":"54.58","rateChangeType":"UP","validFrom":"2026-03-25T09:30:00+09:00","validUntil":"2026-03-25T09:31:00+09:00"}}"#;
        let cal_kr = r#"{"result":{"today":{"date":"2026-03-25","integrated":null},"previousBusinessDay":{"date":"2026-03-24","integrated":null},"nextBusinessDay":{"date":"2026-03-26","integrated":null}}}"#;
        let cal_us = r#"{"result":{"today":{"date":"2026-03-25","dayMarket":null,"preMarket":null,"regularMarket":null,"afterMarket":null},"previousBusinessDay":{"date":"2026-03-24"},"nextBusinessDay":{"date":"2026-03-26"}}}"#;
        let indicators = r#"{"result":[{"symbol":"KOSPI","timestamp":"2026-06-11T15:30:00+09:00","lastPrice":"2812.45"}]}"#;
        let sectors = format!(
            r#"{{"result":{{"sectors":[{{"sectorId":"10","name":"echo-{}","depth":0,"parentSectorId":null}}]}}}}"#,
            sentinels.client_secret
        );
        let quotes = r#"{"result":[{"symbol":"ZZ9901","timestamp":"2026-03-25T09:30:00+09:00","lastPrice":"72000","currency":"KRW"}]}"#;
        let candles = r#"{"result":{"candles":[{"timestamp":"2026-03-25T00:00:00+09:00","openPrice":"71600","highPrice":"72300","lowPrice":"71500","closePrice":"72000","volume":"3521000","currency":"KRW"}],"nextBefore":null}}"#;
        vec![
            http_response(200, "OK", &token),
            http_response(200, "OK", &accounts),
            http_response(200, "OK", &holdings),
            http_response(200, "OK", &bp_krw),
            http_response(200, "OK", bp_usd),
            http_response(200, "OK", &transactions),
            http_response(200, "OK", fx),
            http_response(200, "OK", cal_kr),
            http_response(200, "OK", cal_us),
            http_response(200, "OK", indicators),
            http_response(200, "OK", &sectors),
            http_response(200, "OK", quotes),
            http_response(200, "OK", candles),
        ]
    }

    struct Sentinels {
        client_id: &'static str,
        client_secret: &'static str,
        access_token: &'static str,
        account_no: &'static str,
        account_seq: &'static str,
        amount_krw: &'static str,
        amount_usd: &'static str,
        market_value_krw: &'static str,
        holding_symbol: &'static str,
        holding_name: &'static str,
        quantity: &'static str,
        last_price: &'static str,
        cash_krw: &'static str,
        filled_price: &'static str,
    }

    impl Sentinels {
        fn sample() -> Self {
            Self {
                client_id: "fake-client-id-839201",
                client_secret: "fake-client-secret-774195",
                access_token: "fake-access-token-551902",
                account_no: "00000991234",
                account_seq: "413987",
                amount_krw: "6543210",
                amount_usd: "1553.77",
                market_value_krw: "7654321",
                holding_symbol: "ZZ9901",
                holding_name: "비밀종목이름987",
                quantity: "321.5",
                last_price: "72987",
                cash_krw: "5123456",
                filled_price: "71111",
            }
        }

        fn all_non_public_values(&self) -> [&'static str; 14] {
            [
                self.client_id,
                self.client_secret,
                self.access_token,
                self.account_no,
                self.account_seq,
                self.amount_krw,
                self.amount_usd,
                self.market_value_krw,
                self.holding_symbol,
                self.holding_name,
                self.quantity,
                self.last_price,
                self.cash_krw,
                self.filled_price,
            ]
        }
    }

    #[test]
    fn no_args_and_help_print_usage_without_touching_stdin_or_network() {
        let probe = ConnectionProbe::bind().expect("probe");
        for args in [
            os_args(&[]),
            os_args(&["--help"]),
            os_args(&["-h"]),
            os_args(&["check", "--help"]),
        ] {
            let (reader, reads) = SpyReader::new();
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let (clock, wait) = fake_time();
            let mut ctx = RuntimeContext {
                stdin_kind: StdinKind::Terminal,
                stdin: reader,
                stdout: &mut stdout,
                stderr: &mut stderr,
                lock_path: None,
                repo_roots: Vec::new(),
                clock,
                wait,
                endpoint: None,
            };
            let code = run_async(execute(&args, &mut ctx));
            assert_eq!(code, SmokeExitCode::Success);
            assert_eq!(reads.load(Ordering::SeqCst), 0);
            let out_text = String::from_utf8(stdout).expect("utf8");
            assert!(out_text.contains(EXPECTED_NO_ECHO_COMMAND), "{out_text}");
            assert!(!out_text.contains("--lock-file"), "{out_text}");
            assert!(!out_text.contains("--local-port"), "{out_text}");
            assert_eq!(stderr.len(), 0);
        }
        assert_eq!(
            probe.observe().expect("probe"),
            ConnectionObservation::NoConnection
        );
        assert!(usage_text().contains(CONFIRM_APP_STOPPED_PHRASE));
    }

    #[test]
    fn cli_rejects_credential_options_and_never_echoes_secret_values() {
        let secret = "FAKE_CLI_SECRET_991122";
        let cases = [
            vec!["--client-id", secret],
            vec!["--client-secret", secret],
            vec!["check", "--client-id", secret],
            vec!["check", "--client-secret", secret],
            vec!["check", "--secret=FAKE_CLI_SECRET_991122"],
            vec!["check", "--token", secret],
            vec!["check", "--key", secret],
            vec!["check", "--api-key", secret],
            vec!["check", "--password", secret],
            vec!["check", "--config", secret],
            vec!["check", "--env-file", secret],
            vec!["check", "-k", secret],
            vec!["check", "-s", secret],
            vec!["check", "-t", secret],
            vec!["check", "-p", secret],
        ];
        for bad_args in cases {
            assert_eq!(
                parse_args(&os_args(&bad_args)),
                Err(CliError::ForbiddenSecretOption),
                "{bad_args:?}"
            );
            let (reader, reads) = SpyReader::new();
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            let (clock, wait) = fake_time();
            let mut ctx = RuntimeContext {
                stdin_kind: StdinKind::NonTerminal,
                stdin: reader,
                stdout: &mut stdout,
                stderr: &mut stderr,
                lock_path: None,
                repo_roots: Vec::new(),
                clock,
                wait,
                endpoint: None,
            };
            let code = run_async(execute(&os_args(&bad_args), &mut ctx));
            assert_eq!(code, SmokeExitCode::UsageError);
            assert_eq!(reads.load(Ordering::SeqCst), 0);
            let err_text = String::from_utf8(stderr).expect("utf8");
            assert!(!err_text.contains(secret), "{err_text}");
            assert_eq!(stdout.len(), 0);
        }
    }

    #[test]
    fn cli_rejects_invalid_and_test_only_options_and_accepts_only_record() {
        assert_eq!(
            parse_args(&os_args(&["check", "--local-port", "8080"])),
            Err(CliError::UnknownOption)
        );
        assert_eq!(
            parse_args(&os_args(&["check", "--lock-file", "/tmp/bypass.lock"])),
            Err(CliError::UnknownOption)
        );
        assert_eq!(
            parse_args(&os_args(&["check", "--unknown"])),
            Err(CliError::UnknownOption)
        );
        assert_eq!(
            parse_args(&os_args(&["unknown"])),
            Err(CliError::UnknownCommand)
        );
        assert_eq!(
            parse_args(&os_args(&["check", "extra"])),
            Err(CliError::UnexpectedArgument)
        );
        assert_eq!(
            parse_args(&os_args(&["check", "--record"])),
            Err(CliError::MissingValue)
        );
        assert_eq!(
            parse_args(&os_args(&[
                "check", "--record", "/tmp/a", "--record", "/tmp/b"
            ])),
            Err(CliError::DuplicateOption)
        );
        assert_eq!(
            parse_args(&os_args(&["--help", "--client-secret", "SECRET"])),
            Err(CliError::ForbiddenSecretOption)
        );
        assert_eq!(
            parse_args(&os_args(&["help", "extra"])),
            Err(CliError::UnexpectedArgument)
        );
        assert!(matches_check_only_safe_fields(
            parse_args(&os_args(&["check"])).expect("check")
        ));
    }

    fn matches_check_only_safe_fields(command: Command) -> bool {
        match command {
            Command::Check(opts) => opts.record_path.is_none(),
            Command::Usage => false,
        }
    }

    #[test]
    fn terminal_stdin_fails_closed_with_pinned_single_screen_no_echo_guide() {
        let (reader, reads) = SpyReader::new();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let (clock, wait) = fake_time();
        let mut ctx = RuntimeContext {
            stdin_kind: StdinKind::Terminal,
            stdin: reader,
            stdout: &mut stdout,
            stderr: &mut stderr,
            lock_path: None,
            repo_roots: Vec::new(),
            clock,
            wait,
            endpoint: None,
        };
        let code = run_async(execute(&os_args(&["check"]), &mut ctx));
        assert_eq!(code, SmokeExitCode::TerminalInputRejected);
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        assert_eq!(stdout.len(), 0);
        let err_text = String::from_utf8(stderr).expect("utf8");
        assert!(err_text.contains(EXPECTED_NO_ECHO_COMMAND), "{err_text}");
        assert!(!EXPECTED_NO_ECHO_COMMAND.contains('\n'));
        assert!(!EXPECTED_NO_ECHO_COMMAND.contains("INT"));
        assert!(!EXPECTED_NO_ECHO_COMMAND.contains("TERM"));
        assert!(!err_text.contains("'앱을 껐습니다' |"), "{err_text}");
        let guide = no_echo_guide();
        assert!(guide.contains(EXPECTED_NO_ECHO_COMMAND), "{guide}");
        assert!(guide.lines().count() <= 20, "guide must fit on one screen");
        assert!(usage_text().contains(EXPECTED_NO_ECHO_COMMAND));
    }

    #[test]
    fn stdin_parser_validates_format_bounds_and_redacts_debug() {
        let mut reader = Cursor::new(b"id-123\r\nsec-456\r\n");
        let parsed = read_input(StdinKind::NonTerminal, &mut reader).expect("two lines");
        assert_eq!(parsed.confirmation(), AppStopConfirmation::NotProvided);
        assert_eq!(format!("{parsed:?}"), "SmokeInput(<redacted>)");
        assert!(!format!("{parsed:?}").contains("id-123"));
        assert!(!format!("{parsed:?}").contains("sec-456"));

        let mut empty_third = Cursor::new(b"id-123\nsec-456\n\n");
        let parsed_empty_third =
            read_input(StdinKind::NonTerminal, &mut empty_third).expect("empty third line");
        assert_eq!(
            parsed_empty_third.confirmation(),
            AppStopConfirmation::NotProvided
        );

        let confirmed_bytes = format!("id-123\nsec-456\n{CONFIRM_APP_STOPPED_PHRASE}\n");
        let parsed_confirmed = read_input(
            StdinKind::NonTerminal,
            &mut Cursor::new(confirmed_bytes.as_bytes()),
        )
        .expect("confirmed");
        assert_eq!(
            parsed_confirmed.confirmation(),
            AppStopConfirmation::Confirmed
        );
    }

    #[test]
    fn stdin_parser_rejects_malformed_and_oversized_inputs() {
        let invalid_cases: [(&[u8], InputError); 7] = [
            (b"", InputError::MissingClientId),
            (b"only-id\n", InputError::MissingClientSecret),
            (b"bad id\nsec\n", InputError::InvalidClientId),
            (b"id\nbad\tsec\n", InputError::InvalidClientSecret),
            (b"id\nsec\nwrong-phrase\n", InputError::InvalidConfirmation),
            (b"id\nsec\n\nextra\n", InputError::UnexpectedExtraInput),
            (&[0xFF, 0xFE], InputError::InvalidUtf8),
        ];
        for (raw, expected) in invalid_cases {
            assert_eq!(
                read_input(StdinKind::NonTerminal, &mut Cursor::new(raw)).expect_err("invalid"),
                expected
            );
        }
        let oversized = vec![b'a'; 8193];
        assert_eq!(
            read_input(StdinKind::NonTerminal, &mut Cursor::new(&oversized)).expect_err("size"),
            InputError::TooLarge
        );
    }

    fn hold_and_verify_smoke_lock(lock_path: &Path) {
        let guard =
            acquire_token_lock(lock_path, AppStopConfirmation::Confirmed).expect("confirmed");
        assert_eq!(guard.mode(), LockMode::SmokeConfirmed);
        assert_eq!(
            fs::read_to_string(lock_path).expect("running marker"),
            SMOKE_RUNNING_CONTENT
        );
        assert_eq!(
            fs::metadata(lock_path).expect("meta").permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            acquire_token_lock(lock_path, AppStopConfirmation::Confirmed).expect_err("held"),
            LockError::AppRunning
        );
    }

    fn hold_and_verify_desktop_stopped_lock(lock_path: &Path) {
        let desktop_guard = acquire_token_lock(lock_path, AppStopConfirmation::NotProvided)
            .expect("verified stopped");
        assert_eq!(desktop_guard.mode(), LockMode::DesktopVerifiedStopped);
        assert_eq!(
            acquire_token_lock(lock_path, AppStopConfirmation::Confirmed)
                .expect_err("held by smoke"),
            LockError::AppRunning
        );
    }

    fn hold_external_lock_and_assert_rejected(lock_path: &Path) {
        let held_file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(lock_path)
            .expect("open");
        held_file.try_lock().expect("desktop holds lock");
        assert_eq!(
            acquire_token_lock(lock_path, AppStopConfirmation::Confirmed).expect_err("app running"),
            LockError::AppRunning
        );
    }

    #[test]
    fn lock_contract_enforces_confirmation_when_indeterminate() {
        let dir = TempDirGuard::new("lock-smoke");
        let lock_path = dir.path().join("app-data").join(LOCK_FILE_NAME);
        assert_eq!(
            acquire_token_lock(&lock_path, AppStopConfirmation::NotProvided)
                .expect_err("unconfirmed"),
            LockError::UnconfirmedIndeterminate
        );
        assert!(!lock_path.exists());
        hold_and_verify_smoke_lock(&lock_path);
        assert_eq!(
            fs::read_to_string(&lock_path).expect("stopped marker"),
            SMOKE_STOPPED_CONTENT
        );
        assert_eq!(
            acquire_token_lock(&lock_path, AppStopConfirmation::NotProvided)
                .expect_err("still indeterminate"),
            LockError::UnconfirmedIndeterminate
        );
    }

    #[test]
    fn lock_contract_honors_desktop_marker_blocks_running_app_and_rejects_symlinks() {
        let dir = TempDirGuard::new("lock-desktop");
        let lock_path = dir.path().join("app-data").join(LOCK_FILE_NAME);
        fs::create_dir_all(lock_path.parent().expect("parent")).expect("dir");
        fs::write(&lock_path, DESKTOP_STOPPED_CONTENT).expect("desktop stopped");
        hold_and_verify_desktop_stopped_lock(&lock_path);
        assert_eq!(
            fs::read_to_string(&lock_path).expect("preserved"),
            DESKTOP_STOPPED_CONTENT
        );
        hold_external_lock_and_assert_rejected(&lock_path);
        let target_file = dir.path().join("real.lock");
        fs::write(&target_file, DESKTOP_STOPPED_CONTENT).expect("real");
        let symlink_lock = dir.path().join("link.lock");
        std::os::unix::fs::symlink(&target_file, &symlink_lock).expect("symlink");
        assert_eq!(
            acquire_token_lock(&symlink_lock, AppStopConfirmation::Confirmed).expect_err("symlink"),
            LockError::UnsafePath
        );
        let dangling_lock = dir.path().join("dangling.lock");
        std::os::unix::fs::symlink(dir.path().join("nonexistent.lock"), &dangling_lock)
            .expect("dangling symlink");
        assert_eq!(
            acquire_token_lock(&dangling_lock, AppStopConfirmation::Confirmed)
                .expect_err("dangling symlink"),
            LockError::UnsafePath
        );
    }

    fn prepare_and_check_permissions(valid: &Path, repo_roots: &[PathBuf]) {
        let _handle = prepare_recording(valid, repo_roots).expect("outside ok");
        assert_eq!(
            fs::metadata(valid).expect("meta").permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn record_guard_rejects_inside_repo_symlinks_and_existing_files() {
        let dir = TempDirGuard::new("record");
        let fake_repo = dir.path().join("repo");
        fs::create_dir_all(fake_repo.join(".git")).expect("git dir");
        fs::create_dir_all(fake_repo.join("sub")).expect("sub dir");
        let outside = dir.path().join("outside");
        fs::create_dir_all(&outside).expect("outside dir");

        let repo_roots = detect_repo_roots(&fake_repo.join("sub"));
        assert_ne!(repo_roots.len(), 0);

        assert_eq!(
            prepare_recording(&fake_repo.join("sub/out.json"), &repo_roots).expect_err("in repo"),
            RecordError::InsideRepository
        );
        assert_eq!(
            prepare_recording(&fake_repo.join("sub/out.json"), &[]).expect_err("in git ancestor"),
            RecordError::InsideRepository
        );
        let escaped = outside.join("../repo/sub/escaped.json");
        assert_eq!(
            prepare_recording(&escaped, &repo_roots).expect_err("dotdot escape"),
            RecordError::InsideRepository
        );
        let symlink_to_repo = outside.join("link-to-repo");
        std::os::unix::fs::symlink(&fake_repo, &symlink_to_repo).expect("symlink");
        assert_eq!(
            prepare_recording(&symlink_to_repo.join("out.json"), &repo_roots).expect_err("symlink"),
            RecordError::InsideRepository
        );

        let existing = outside.join("exists.json");
        fs::write(&existing, "{}").expect("exists");
        assert_eq!(
            prepare_recording(&existing, &repo_roots).expect_err("exists"),
            RecordError::AlreadyExists
        );
        assert_eq!(
            prepare_recording(&outside.join("missing/out.json"), &repo_roots)
                .expect_err("missing parent"),
            RecordError::MissingParent
        );

        let plain_forbidden = dir.path().join("plain-forbidden");
        fs::create_dir_all(&plain_forbidden).expect("plain dir");
        assert_eq!(
            prepare_recording(
                &plain_forbidden.join("out.json"),
                std::slice::from_ref(&plain_forbidden)
            )
            .expect_err("explicit forbidden root"),
            RecordError::InsideRepository
        );
        let valid = outside.join("ok.json");
        prepare_and_check_permissions(&valid, &repo_roots);
        assert!(!valid.exists(), "uncommitted placeholder removed on drop");
    }

    fn assert_expected_wire_requests(wire: &str, sentinels: &Sentinels) {
        let request_lines: Vec<&str> = wire
            .lines()
            .map(|line| line.trim_end_matches('\r'))
            .filter(|line| line.ends_with(" HTTP/1.1"))
            .collect();
        let expected_second = format!(
            "grant_type=client_credentials&client_id={}&client_secret={}GET /api/v1/accounts HTTP/1.1",
            sentinels.client_id, sentinels.client_secret
        );
        assert_eq!(
            request_lines,
            vec![
                "POST /oauth2/token HTTP/1.1",
                expected_second.as_str(),
                "GET /api/v1/holdings HTTP/1.1",
                "GET /api/v1/buying-power?currency=KRW HTTP/1.1",
                "GET /api/v1/buying-power?currency=USD HTTP/1.1",
                "GET /api/v1/transactions?limit=5 HTTP/1.1",
                "GET /api/v1/exchange-rate?baseCurrency=USD&quoteCurrency=KRW HTTP/1.1",
                "GET /api/v1/market-calendar/KR HTTP/1.1",
                "GET /api/v1/market-calendar/US HTTP/1.1",
                "GET /api/v1/market-indicators/prices?symbols=KOSPI%2CKOSDAQ HTTP/1.1",
                "GET /api/v1/sectors HTTP/1.1",
                "GET /api/v1/prices?symbols=ZZ9901 HTTP/1.1",
                "GET /api/v1/candles?symbol=ZZ9901&interval=1d&count=5&adjusted=true HTTP/1.1",
            ]
        );
        assert!(wire.contains("x-tossinvest-account: 413987"));
    }

    fn assert_no_leaks_in_outputs_and_recording(
        sentinels: &Sentinels,
        out_text: &str,
        err_text: &str,
        lock_path: &Path,
        record_path: &Path,
    ) {
        let lock_text = fs::read_to_string(lock_path).expect("lock text");
        for secret in sentinels.all_non_public_values() {
            assert!(
                !out_text.contains(secret),
                "stdout leaked {secret}:\n{out_text}"
            );
            assert!(
                !err_text.contains(secret),
                "stderr leaked {secret}:\n{err_text}"
            );
            assert!(
                !lock_text.contains(secret),
                "lock leaked {secret}:\n{lock_text}"
            );
        }
        assert!(out_text.contains("성공 13, 실패 0, 건너뜀 0"), "{out_text}");
        assert!(out_text.contains("스펙 일치"), "{out_text}");
        assert!(out_text.contains("BROKERAGE"), "{out_text}");

        let rec_text = fs::read_to_string(record_path).expect("record text");
        assert!(
            !rec_text.contains(sentinels.client_id),
            "recording leaked client_id"
        );
        assert!(
            !rec_text.contains(sentinels.client_secret),
            "recording leaked client_secret"
        );
        assert!(
            !rec_text.contains(sentinels.access_token),
            "recording leaked access_token"
        );
        assert_eq!(
            fs::metadata(record_path)
                .expect("rec meta")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let rec_json: Value = serde_json::from_str(&rec_text).expect("record JSON");
        assert_eq!(
            rec_json
                .pointer("/steps/0/body/access_token")
                .and_then(Value::as_str),
            Some("[REDACTED]")
        );
        assert_eq!(
            rec_json
                .pointer("/steps/0/body/expires_in")
                .and_then(Value::as_u64),
            Some(86400)
        );
        assert_eq!(
            rec_json
                .pointer("/steps/10/body/result/sectors/0/name")
                .and_then(Value::as_str),
            Some("echo-[REDACTED]")
        );
    }

    #[test]
    fn full_happy_path_verifies_all_steps_redacts_output_and_scrubs_recording() {
        let dir = TempDirGuard::new("happy");
        let lock_path = dir.path().join("app").join(LOCK_FILE_NAME);
        let record_path = dir.path().join("t13-record.json");
        let sentinels = Sentinels::sample();
        let server = ScriptedHttpServer::start(happy_responses(&sentinels)).expect("server");
        let port = server.port();
        let stdin_data = format!(
            "{}\n{}\n{}\n",
            sentinels.client_id, sentinels.client_secret, CONFIRM_APP_STOPPED_PHRASE
        );
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let (clock, wait) = fake_time();
        let mut ctx = RuntimeContext {
            stdin_kind: StdinKind::NonTerminal,
            stdin: Cursor::new(stdin_data.into_bytes()),
            stdout: &mut stdout,
            stderr: &mut stderr,
            lock_path: Some(lock_path.clone()),
            repo_roots: Vec::new(),
            clock,
            wait,
            endpoint: TossEndpoint::local(port).ok(),
        };
        let args = os_args(&[
            "check",
            "--record",
            record_path.to_str().expect("record path"),
        ]);
        let code = run_async(execute(&args, &mut ctx));
        let wire_bytes = server.finish().expect("server finish");
        let wire = String::from_utf8_lossy(&wire_bytes);

        assert_eq!(code, SmokeExitCode::Success);
        assert_expected_wire_requests(&wire, &sentinels);
        let out_text = String::from_utf8(stdout).expect("stdout utf8");
        let err_text = String::from_utf8(stderr).expect("stderr utf8");
        assert_no_leaks_in_outputs_and_recording(
            &sentinels,
            &out_text,
            &err_text,
            &lock_path,
            &record_path,
        );
    }

    fn run_scripted_with_stopped_lock(
        lock_path: &Path,
        responses: Vec<Vec<u8>>,
    ) -> (SmokeExitCode, String, String) {
        let server = ScriptedHttpServer::start(responses).expect("server");
        let port = server.port();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let (clock, wait) = fake_time();
        let mut ctx = RuntimeContext {
            stdin_kind: StdinKind::NonTerminal,
            stdin: Cursor::new(b"id-abc\nsec-xyz\n".to_vec()),
            stdout: &mut stdout,
            stderr: &mut stderr,
            lock_path: Some(lock_path.to_path_buf()),
            repo_roots: Vec::new(),
            clock,
            wait,
            endpoint: TossEndpoint::local(port).ok(),
        };
        let code = run_async(execute(&os_args(&["check"]), &mut ctx));
        let wire = String::from_utf8_lossy(&server.finish().expect("finish")).into_owned();
        (code, String::from_utf8(stdout).expect("utf8"), wire)
    }

    fn stopped_lock_file(dir: &TempDirGuard) -> PathBuf {
        let lock_path = dir.path().join("desktop").join(LOCK_FILE_NAME);
        fs::create_dir_all(lock_path.parent().expect("parent")).expect("dir");
        fs::write(&lock_path, DESKTOP_STOPPED_CONTENT).expect("stopped");
        lock_path
    }

    #[test]
    fn failure_stages_return_distinct_exit_codes_and_halt_appropriately() {
        let dir = TempDirGuard::new("stages");
        let lock_path = stopped_lock_file(&dir);

        let (token_code, token_out, _) = run_scripted_with_stopped_lock(
            &lock_path,
            vec![http_response(
                401,
                "Unauthorized",
                r#"{"error":"invalid_client","requestId":"oauth-req-1"}"#,
            )],
        );
        assert_eq!(token_code, SmokeExitCode::TokenStepFailed);
        assert!(token_out.contains("AccessDenied/InvalidCredentials"));
        assert!(token_out.contains("requestId=oauth-req-1"));

        let (accounts_code, accounts_out, _) = run_scripted_with_stopped_lock(
            &lock_path,
            vec![
                http_response(
                    200,
                    "OK",
                    r#"{"access_token":"tok1","token_type":"Bearer","expires_in":3600}"#,
                ),
                http_response(
                    403,
                    "Forbidden",
                    r#"{"error":{"code":"ip-not-allowed","requestId":"acc-req-2","referenceId":"acc-ref-2"}}"#,
                ),
            ],
        );
        assert_eq!(accounts_code, SmokeExitCode::AccountsStepFailed);
        assert!(accounts_out.contains("AccessDenied/IpNotAllowed"));
        assert!(accounts_out.contains("referenceId=acc-ref-2"));
    }

    #[test]
    fn holdings_and_additional_failures_report_distinct_exit_codes() {
        let dir = TempDirGuard::new("stages-more");
        let lock_path = stopped_lock_file(&dir);

        let (holdings_401_code, holdings_out, _) = run_scripted_with_stopped_lock(
            &lock_path,
            vec![
                http_response(
                    200,
                    "OK",
                    r#"{"access_token":"tok1","token_type":"Bearer","expires_in":3600}"#,
                ),
                http_response(
                    200,
                    "OK",
                    r#"{"result":[{"accountNo":"1","accountSeq":7,"accountType":"BROKERAGE"}]}"#,
                ),
                http_response(
                    401,
                    "Unauthorized",
                    r#"{"error":{"code":"token-revoked","requestId":"h-401"}}"#,
                ),
            ],
        );
        assert_eq!(holdings_401_code, SmokeExitCode::HoldingsStepFailed);
        assert!(holdings_out.contains("이전 단계 인증 실패(401)로 중단"));

        let mut additional_fail = happy_responses(&Sentinels::sample());
        if let Some(slot) = additional_fail.get_mut(10) {
            *slot = http_response(200, "OK", r#"{"result":{"sectors":"not-an-array"}}"#);
        }
        let (add_code, add_out, _) = run_scripted_with_stopped_lock(&lock_path, additional_fail);
        assert_eq!(add_code, SmokeExitCode::AdditionalStepFailed);
        assert!(add_out.contains("스펙 불일치"));
    }

    #[test]
    fn empty_accounts_skips_account_steps_and_still_runs_market_steps() {
        let dir = TempDirGuard::new("empty-acc");
        let lock_path = stopped_lock_file(&dir);
        let responses = vec![
            http_response(
                200,
                "OK",
                r#"{"access_token":"tok1","token_type":"Bearer","expires_in":3600}"#,
            ),
            http_response(200, "OK", r#"{"result":[]}"#),
            http_response(
                200,
                "OK",
                r#"{"result":{"baseCurrency":"USD","quoteCurrency":"KRW","rate":"1381.7","midRate":"1374.2","basisPoint":"54.58","rateChangeType":"UP","validFrom":"2026-03-25T09:30:00+09:00","validUntil":"2026-03-25T09:31:00+09:00"}}"#,
            ),
            http_response(
                200,
                "OK",
                r#"{"result":{"today":{"date":"2026-03-25"},"previousBusinessDay":{"date":"2026-03-24"},"nextBusinessDay":{"date":"2026-03-26"}}}"#,
            ),
            http_response(
                200,
                "OK",
                r#"{"result":{"today":{"date":"2026-03-25"},"previousBusinessDay":{"date":"2026-03-24"},"nextBusinessDay":{"date":"2026-03-26"}}}"#,
            ),
            http_response(
                200,
                "OK",
                r#"{"result":[{"symbol":"KOSPI","lastPrice":"2812.45"}]}"#,
            ),
            http_response(200, "OK", r#"{"result":{"sectors":[]}}"#),
            http_response(
                200,
                "OK",
                r#"{"result":[{"symbol":"005930","lastPrice":"72000","currency":"KRW"}]}"#,
            ),
            http_response(200, "OK", r#"{"result":{"candles":[],"nextBefore":null}}"#),
        ];
        let (code, out_text, wire) = run_scripted_with_stopped_lock(&lock_path, responses);
        assert_eq!(code, SmokeExitCode::Success);
        assert!(wire.contains("GET /api/v1/prices?symbols=005930 HTTP/1.1"));
        assert!(out_text.contains("성공 9, 실패 0, 건너뜀 4"), "{out_text}");
        assert!(out_text.contains("계좌 없음으로 건너뜀"), "{out_text}");
    }

    #[test]
    fn record_inside_repo_blocks_before_any_network_call() {
        let dir = TempDirGuard::new("early-record");
        let repo = dir.path().join("repo");
        fs::create_dir_all(repo.join(".git")).expect("git");
        let lock_path = stopped_lock_file(&dir);
        let probe = ConnectionProbe::bind().expect("probe");
        let port = probe.port().expect("port");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let (clock, wait) = fake_time();
        let mut ctx = RuntimeContext {
            stdin_kind: StdinKind::NonTerminal,
            stdin: Cursor::new(b"id-1\nsec-1\n".to_vec()),
            stdout: &mut stdout,
            stderr: &mut stderr,
            lock_path: Some(lock_path),
            repo_roots: vec![repo.clone()],
            clock,
            wait,
            endpoint: TossEndpoint::local(port).ok(),
        };
        let inside = repo.join("leak.json");
        let code = run_async(execute(
            &os_args(&["check", "--record", inside.to_str().expect("str")]),
            &mut ctx,
        ));
        assert_eq!(code, SmokeExitCode::RecordPathRejected);
        assert_eq!(
            probe.observe().expect("probe"),
            ConnectionObservation::NoConnection
        );
    }

    #[test]
    fn running_app_blocks_before_any_network_call_even_with_confirmation() {
        let dir = TempDirGuard::new("early-lock");
        let lock_path = stopped_lock_file(&dir);
        let probe = ConnectionProbe::bind().expect("probe");
        let port = probe.port().expect("port");
        let held = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .expect("open");
        held.try_lock().expect("hold");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let (clock, wait) = fake_time();
        let mut ctx = RuntimeContext {
            stdin_kind: StdinKind::NonTerminal,
            stdin: Cursor::new(format!("id-1\nsec-1\n{CONFIRM_APP_STOPPED_PHRASE}\n").into_bytes()),
            stdout: &mut stdout,
            stderr: &mut stderr,
            lock_path: Some(lock_path),
            repo_roots: Vec::new(),
            clock,
            wait,
            endpoint: TossEndpoint::local(port).ok(),
        };
        let code = run_async(execute(&os_args(&["check"]), &mut ctx));
        assert_eq!(code, SmokeExitCode::AppRunning);
        assert_eq!(
            probe.observe().expect("probe"),
            ConnectionObservation::NoConnection
        );
    }

    #[test]
    fn exit_codes_are_all_unique_and_labeled() {
        let all = [
            SmokeExitCode::Success,
            SmokeExitCode::UsageError,
            SmokeExitCode::TerminalInputRejected,
            SmokeExitCode::InputInvalid,
            SmokeExitCode::AppRunning,
            SmokeExitCode::AppStateUnconfirmed,
            SmokeExitCode::RecordPathRejected,
            SmokeExitCode::LockIoError,
            SmokeExitCode::RuntimeError,
            SmokeExitCode::TokenStepFailed,
            SmokeExitCode::AccountsStepFailed,
            SmokeExitCode::HoldingsStepFailed,
            SmokeExitCode::AdditionalStepFailed,
            SmokeExitCode::RecordWriteFailed,
        ];
        let unique: std::collections::BTreeSet<u8> = all.iter().map(|code| code.as_u8()).collect();
        assert_eq!(unique.len(), all.len());
        for code in all {
            assert_ne!(code.label().len(), 0);
        }
    }

    #[test]
    fn lock_rejects_crashed_desktop_without_confirmation_and_symlink_parent() {
        let dir = TempDirGuard::new("lock-edge");
        let lock_path = dir.path().join("app").join(LOCK_FILE_NAME);
        fs::create_dir_all(lock_path.parent().expect("parent")).expect("dir");
        fs::write(&lock_path, DESKTOP_RUNNING_CONTENT).expect("crashed");
        assert_eq!(
            acquire_token_lock(&lock_path, AppStopConfirmation::NotProvided)
                .expect_err("crashed desktop unconfirmed"),
            LockError::UnconfirmedIndeterminate
        );
        hold_and_verify_smoke_lock(&lock_path);

        let real_dir = dir.path().join("real-dir");
        fs::create_dir_all(&real_dir).expect("real dir");
        let sym_dir = dir.path().join("sym-dir");
        std::os::unix::fs::symlink(&real_dir, &sym_dir).expect("sym dir");
        assert_eq!(
            acquire_token_lock(
                &sym_dir.join(LOCK_FILE_NAME),
                AppStopConfirmation::Confirmed
            )
            .expect_err("symlink parent"),
            LockError::UnsafePath
        );
    }

    #[test]
    fn lock_refuses_unrecognized_existing_file_and_hardlink_without_modification() {
        let dir = TempDirGuard::new("lock-protect");
        let victim = dir.path().join("victim.txt");
        let original = "IMPORTANT USER DATA DO NOT TOUCH\n";
        fs::write(&victim, original).expect("write victim");
        fs::set_permissions(&victim, Permissions::from_mode(0o644)).expect("chmod");
        assert_eq!(
            acquire_token_lock(&victim, AppStopConfirmation::Confirmed).expect_err("victim"),
            LockError::UnsafePath
        );
        assert_eq!(fs::read_to_string(&victim).expect("read victim"), original);
        assert_eq!(
            fs::metadata(&victim).expect("meta").permissions().mode() & 0o777,
            0o644
        );

        let target = dir.path().join("orig.lock");
        let linked = dir.path().join("hard.lock");
        fs::write(&target, SMOKE_STOPPED_CONTENT).expect("write target");
        fs::set_permissions(&target, Permissions::from_mode(0o644)).expect("chmod target");
        fs::hard_link(&target, &linked).expect("hard link");
        assert_eq!(
            acquire_token_lock(&linked, AppStopConfirmation::Confirmed).expect_err("hardlink"),
            LockError::UnsafePath
        );
        assert_eq!(
            fs::read_to_string(&target).expect("read target"),
            SMOKE_STOPPED_CONTENT
        );
        assert_eq!(
            fs::metadata(&target).expect("meta").permissions().mode() & 0o777,
            0o644
        );
    }

    #[test]
    fn lock_contract_constants_match_adr_literals_and_tauri_identifier() {
        assert_eq!(APP_DATA_DIR_NAME, "dev.stockcopilot.desktop");
        assert_eq!(LOCK_FILE_NAME, "toss-token.lock");
        assert_eq!(
            DESKTOP_RUNNING_CONTENT,
            "version=1\nowner=desktop-app\nstate=running\n"
        );
        assert_eq!(
            DESKTOP_STOPPED_CONTENT,
            "version=1\nowner=desktop-app\nstate=stopped\n"
        );
        assert_eq!(
            SMOKE_RUNNING_CONTENT,
            "version=1\nowner=toss-smoke\nstate=running\n"
        );
        assert_eq!(
            SMOKE_STOPPED_CONTENT,
            "version=1\nowner=toss-smoke\nstate=stopped\n"
        );

        let tauri_conf_text =
            fs::read_to_string("../../apps/desktop/src-tauri/tauri.conf.json").expect("tauri conf");
        let tauri_conf: Value = serde_json::from_str(&tauri_conf_text).expect("tauri json");
        assert_eq!(
            tauri_conf.get("identifier").and_then(Value::as_str),
            Some(APP_DATA_DIR_NAME)
        );
    }

    #[test]
    fn lock_tightens_existing_parent_permissions_and_rejects_empty_lock_file() {
        let dir = TempDirGuard::new("lock-parent-mode");
        let existing_desktop_parent = dir.path().join("desktop-dir");
        fs::create_dir_all(&existing_desktop_parent).expect("create desktop dir");
        fs::set_permissions(&existing_desktop_parent, Permissions::from_mode(0o755))
            .expect("chmod 755");
        let desktop_lock = existing_desktop_parent.join("toss-token.lock");
        fs::write(
            &desktop_lock,
            "version=1\nowner=desktop-app\nstate=stopped\n",
        )
        .expect("write desktop stopped");
        let _desktop_guard =
            acquire_token_lock(&desktop_lock, AppStopConfirmation::NotProvided).expect("desktop");
        assert_eq!(
            fs::metadata(&existing_desktop_parent)
                .expect("meta")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );

        let existing_new_parent = dir.path().join("new-dir");
        fs::create_dir_all(&existing_new_parent).expect("create new dir");
        fs::set_permissions(&existing_new_parent, Permissions::from_mode(0o755))
            .expect("chmod 755");
        let new_lock = existing_new_parent.join("toss-token.lock");
        let _smoke_guard =
            acquire_token_lock(&new_lock, AppStopConfirmation::Confirmed).expect("smoke");
        assert_eq!(
            fs::metadata(&existing_new_parent)
                .expect("meta")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );

        let empty_parent = dir.path().join("empty-dir");
        fs::create_dir_all(&empty_parent).expect("create empty dir");
        fs::set_permissions(&empty_parent, Permissions::from_mode(0o755)).expect("chmod 755");
        let empty_lock = empty_parent.join("toss-token.lock");
        fs::write(&empty_lock, "").expect("write empty");
        assert_eq!(
            acquire_token_lock(&empty_lock, AppStopConfirmation::Confirmed).expect_err("empty"),
            LockError::UnsafePath
        );
        assert!(LockError::UnsafePath.message().contains("0바이트"));
        assert_eq!(
            fs::metadata(&empty_parent)
                .expect("meta")
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
    }

    #[test]
    fn missing_default_lock_path_exits_with_lock_io_error_before_network() {
        let probe = ConnectionProbe::bind().expect("probe");
        let port = probe.port().expect("port");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let (clock, wait) = fake_time();
        let mut ctx = RuntimeContext {
            stdin_kind: StdinKind::NonTerminal,
            stdin: Cursor::new(b"id-1\nsec-1\n".to_vec()),
            stdout: &mut stdout,
            stderr: &mut stderr,
            lock_path: None,
            repo_roots: Vec::new(),
            clock,
            wait,
            endpoint: TossEndpoint::local(port).ok(),
        };
        let code = run_async(execute(&os_args(&["check"]), &mut ctx));
        assert_eq!(code, SmokeExitCode::LockIoError);
        assert_eq!(
            probe.observe().expect("probe"),
            ConnectionObservation::NoConnection
        );
    }

    #[test]
    fn percent_encoded_secrets_in_response_and_headers_are_scrubbed_from_recording() {
        let dir = TempDirGuard::new("record-pct");
        let record_path = dir.path().join("pct.json");
        let mut handle = prepare_recording(&record_path, &[]).expect("prepare");
        let secret = application::SecretValue::new(b"sec+a=b&c".to_vec());
        let step = toss_smoke::record::RecordedStep {
            step: "sectors",
            path: "/api/v1/sectors",
            status: Some(200),
            outcome: "ok",
            schema_status: "matched",
            rate_limit: Some(toss_smoke::record::RecordedRateLimit {
                limit: Some(String::from("x-sec%2Ba%3Db%26c-y")),
                remaining: Some(String::from("19")),
                reset: None,
                retry_after: None,
            }),
            body: Some(serde_json::json!({
                "raw": "a sec+a=b&c z",
                "upper": "a sec%2Ba%3Db%26c z",
                "lower": "a sec%2ba%3db%26c z"
            })),
        };
        handle.write_all(&[step], &[&secret]).expect("write");
        let written = fs::read_to_string(&record_path).expect("read");
        assert!(!written.contains("sec+a=b&c"));
        assert!(!written.contains("sec%2Ba%3Db%26c"));
        assert!(!written.contains("sec%2ba%3db%26c"));
        assert!(written.contains("[REDACTED]"));
    }

    #[test]
    fn non_401_holdings_failure_continues_remaining_steps_and_returns_holdings_code() {
        let dir = TempDirGuard::new("holdings-500");
        let lock_path = stopped_lock_file(&dir);
        let mut responses = happy_responses(&Sentinels::sample());
        if let Some(slot) = responses.get_mut(2) {
            *slot = http_response(
                500,
                "Internal Server Error",
                r#"{"error":{"code":"server-error","requestId":"h-500"}}"#,
            );
        }
        if let Some(slot) = responses.get_mut(10) {
            *slot = http_response(200, "OK", r#"{"result":{"sectors":"bad"}}"#);
        }
        let (code, out_text, wire) = run_scripted_with_stopped_lock(&lock_path, responses);
        assert_eq!(code, SmokeExitCode::HoldingsStepFailed);
        assert!(out_text.contains("성공 11, 실패 2, 건너뜀 0"), "{out_text}");
        assert!(wire.contains("GET /api/v1/prices?symbols=005930 HTTP/1.1"));
    }

    #[test]
    fn invalid_json_token_response_fails_with_token_stage_exit_code() {
        let dir = TempDirGuard::new("bad-json-tok");
        let lock_path = stopped_lock_file(&dir);
        let (code, out_text, _) =
            run_scripted_with_stopped_lock(&lock_path, vec![http_response(200, "OK", "not-json")]);
        assert_eq!(code, SmokeExitCode::TokenStepFailed);
        assert!(out_text.contains("유효한 JSON이 아님"), "{out_text}");
    }

    #[test]
    fn error_diagnostic_ids_containing_secrets_are_redacted_in_stdout() {
        let dir = TempDirGuard::new("diag-redact");
        let lock_path = stopped_lock_file(&dir);
        let (code, out_text, _) = run_scripted_with_stopped_lock(
            &lock_path,
            vec![http_response(
                401,
                "Unauthorized",
                r#"{"error":"invalid_client","requestId":"req-sec-xyz-end","referenceId":"ref-id-abc-end"}"#,
            )],
        );
        assert_eq!(code, SmokeExitCode::TokenStepFailed);
        assert!(!out_text.contains("sec-xyz"), "{out_text}");
        assert!(!out_text.contains("id-abc"), "{out_text}");
        assert!(out_text.contains("requestId=[REDACTED]"), "{out_text}");
        assert!(out_text.contains("referenceId=[REDACTED]"), "{out_text}");
    }

    #[test]
    fn transport_failure_on_token_step_returns_token_step_failed() {
        let dir = TempDirGuard::new("transport-fail");
        let lock_path = stopped_lock_file(&dir);
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        drop_listener(listener);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let (clock, wait) = fake_time();
        let mut ctx = RuntimeContext {
            stdin_kind: StdinKind::NonTerminal,
            stdin: Cursor::new(b"id-abc\nsec-xyz\n".to_vec()),
            stdout: &mut stdout,
            stderr: &mut stderr,
            lock_path: Some(lock_path),
            repo_roots: Vec::new(),
            clock,
            wait,
            endpoint: TossEndpoint::local(port).ok(),
        };
        let code = run_async(execute(&os_args(&["check"]), &mut ctx));
        assert_eq!(code, SmokeExitCode::TokenStepFailed);
        let out_text = String::from_utf8(stdout).expect("utf8");
        assert!(out_text.contains("네트워크 전송 실패"), "{out_text}");
    }

    #[test]
    fn stdout_write_failure_returns_runtime_error_instead_of_success() {
        let (reader, _reads) = SpyReader::new();
        let mut stderr = Vec::new();
        let (clock, wait) = fake_time();
        let mut usage_ctx = RuntimeContext {
            stdin_kind: StdinKind::Terminal,
            stdin: reader,
            stdout: FailingWriter,
            stderr: &mut stderr,
            lock_path: None,
            repo_roots: Vec::new(),
            clock,
            wait,
            endpoint: None,
        };
        assert_eq!(
            run_async(execute(&os_args(&[]), &mut usage_ctx)),
            SmokeExitCode::RuntimeError
        );

        let dir = TempDirGuard::new("broken-stdout");
        let lock_path = stopped_lock_file(&dir);
        let sentinels = Sentinels::sample();
        let server = ScriptedHttpServer::start(happy_responses(&sentinels)).expect("server");
        let port = server.port();
        let mut check_stderr = Vec::new();
        let (clock2, wait2) = fake_time();
        let mut check_ctx = RuntimeContext {
            stdin_kind: StdinKind::NonTerminal,
            stdin: Cursor::new(
                format!("{}\n{}\n", sentinels.client_id, sentinels.client_secret).into_bytes(),
            ),
            stdout: FailingWriter,
            stderr: &mut check_stderr,
            lock_path: Some(lock_path),
            repo_roots: Vec::new(),
            clock: clock2,
            wait: wait2,
            endpoint: TossEndpoint::local(port).ok(),
        };
        assert_eq!(
            run_async(execute(&os_args(&["check"]), &mut check_ctx)),
            SmokeExitCode::RuntimeError
        );
        let _wire = server.finish().expect("finish");
    }

    fn drop_listener(listener: std::net::TcpListener) {
        let _closed = listener;
    }
}
