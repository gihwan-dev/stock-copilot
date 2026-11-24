use application::{
    AccessDenial, MissingData, PortErrorKind, ReadEndpoint, RequestRejection, ResponseMismatch,
    RetryDelay, SecretValue, UnavailableCause,
};
use broker_toss::{
    DiagnosticId, QueryKey, RawResponse, ReadRequest, ReadRoute, TossHttpClient, TossHttpError,
    TossMappedError, issue_token,
};
use serde_json::{Map, Value};

use crate::exit_code::SmokeExitCode;
use crate::input::SmokeInput;
use crate::record::{RecordedRateLimit, RecordedStep};
use crate::shape::{EndpointKind, ShapeSummary, SpecMatch, validate_response};

const DEFAULT_PROBE_SYMBOL: &str = "005930";
const REDACTED_TOKEN: &str = "[REDACTED]";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStage {
    Token,
    Accounts,
    Holdings,
    Additional,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureDetail {
    pub category: &'static str,
    pub request_id: Option<String>,
    pub reference_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepOutcome {
    Succeeded(ShapeSummary),
    SchemaMismatch(ShapeSummary),
    HttpFailure(FailureDetail),
    TransportFailure(&'static str),
    Skipped(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepReport {
    pub id: &'static str,
    pub title: &'static str,
    pub path: &'static str,
    pub stage: StepStage,
    pub status: Option<u16>,
    pub outcome: StepOutcome,
}

pub struct RunResult {
    reports: Vec<StepReport>,
    recordings: Vec<RecordedStep>,
    exit_code: SmokeExitCode,
    access_token: Option<SecretValue>,
}

impl std::fmt::Debug for RunResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunResult")
            .field("reports", &self.reports)
            .field("exit_code", &self.exit_code)
            .finish_non_exhaustive()
    }
}

impl RunResult {
    #[must_use]
    pub fn reports(&self) -> &[StepReport] {
        &self.reports
    }

    #[must_use]
    pub fn recordings(&self) -> &[RecordedStep] {
        &self.recordings
    }

    #[must_use]
    pub const fn exit_code(&self) -> SmokeExitCode {
        self.exit_code
    }

    #[must_use]
    pub const fn access_token(&self) -> Option<&SecretValue> {
        self.access_token.as_ref()
    }
}

fn http_error_label(error: TossHttpError) -> &'static str {
    match error {
        TossHttpError::InvalidEndpoint => "잘못된 엔드포인트",
        TossHttpError::InvalidCredential => "헤더에 넣을 수 없는 자격 증명 형식",
        TossHttpError::Configuration => "HTTP 클라이언트 구성 실패",
        TossHttpError::Transport => "네트워크 전송 실패",
        TossHttpError::ResponseTooLarge => "응답 크기 상한(8MiB) 초과",
        TossHttpError::RateLimited => "로컬 속도 제한 초과",
    }
}

fn access_denial_label(cause: AccessDenial) -> &'static str {
    match cause {
        AccessDenial::InvalidCredentials => "AccessDenied/InvalidCredentials",
        AccessDenial::IpNotAllowed => "AccessDenied/IpNotAllowed",
        AccessDenial::Forbidden => "AccessDenied/Forbidden",
        AccessDenial::AccountNotFound => "AccessDenied/AccountNotFound",
        AccessDenial::LoginUserNotFound => "AccessDenied/LoginUserNotFound",
        AccessDenial::TokenRevokedRepeated => "AccessDenied/TokenRevokedRepeated",
    }
}

fn request_rejection_label(cause: RequestRejection) -> &'static str {
    match cause {
        RequestRejection::MalformedRequest => "InvalidRequest/MalformedRequest",
        RequestRejection::MissingAccountHeader => "InvalidRequest/MissingAccountHeader",
        RequestRejection::UnsupportedQuery => "InvalidRequest/UnsupportedQuery",
        RequestRejection::InvalidToken => "InvalidRequest/InvalidToken",
        RequestRejection::EdgeBlocked => "InvalidRequest/EdgeBlocked",
        RequestRejection::UnsupportedContentType => "InvalidRequest/UnsupportedContentType",
        RequestRejection::BatchLimitExceeded => "InvalidRequest/BatchLimitExceeded",
    }
}

fn port_error_label(kind: PortErrorKind) -> &'static str {
    match kind {
        PortErrorKind::AccessDenied(cause) => access_denial_label(cause),
        PortErrorKind::InvalidRequest(cause) => request_rejection_label(cause),
        PortErrorKind::NotFound(MissingData::Stock) => "NotFound/Stock",
        PortErrorKind::NotFound(MissingData::ExchangeRate) => "NotFound/ExchangeRate",
        PortErrorKind::NotFound(MissingData::Sector) => "NotFound/Sector",
        PortErrorKind::NotFound(MissingData::Order) => "NotFound/Order",
        PortErrorKind::Unavailable {
            cause: UnavailableCause::Network,
            ..
        } => "Unavailable/Network",
        PortErrorKind::Unavailable {
            cause: UnavailableCause::RateLimited,
            ..
        } => "Unavailable/RateLimited",
        PortErrorKind::Unavailable {
            cause: UnavailableCause::ServerError,
            ..
        } => "Unavailable/ServerError",
        PortErrorKind::Unavailable {
            cause: UnavailableCause::Maintenance,
            ..
        } => "Unavailable/Maintenance",
        PortErrorKind::Unavailable {
            cause: UnavailableCause::ExpiredToken,
            ..
        } => "Unavailable/ExpiredToken",
        PortErrorKind::InvalidResponse(ResponseMismatch::Schema) => "InvalidResponse/Schema",
        PortErrorKind::InvalidResponse(ResponseMismatch::Validation) => {
            "InvalidResponse/Validation"
        }
        PortErrorKind::MasterNotReady(_) => "MasterNotReady",
        PortErrorKind::Unsupported(_) => "Unsupported",
    }
}

fn contains_any_secret(text: &str, secrets: &[&SecretValue]) -> bool {
    secrets.iter().any(|secret| {
        std::str::from_utf8(secret.as_exposed_secret())
            .ok()
            .filter(|needle| !needle.is_empty())
            .is_some_and(|needle| text.contains(needle))
    })
}

fn sanitize_diagnostic(raw: Option<&str>, secrets: &[&SecretValue]) -> Option<String> {
    let text = raw?;
    if contains_any_secret(text, secrets) {
        Some(String::from(REDACTED_TOKEN))
    } else {
        Some(text.to_owned())
    }
}

fn failure_detail(mapped: &TossMappedError, secrets: &[&SecretValue]) -> FailureDetail {
    FailureDetail {
        category: port_error_label(mapped.port_error().kind()),
        request_id: sanitize_diagnostic(
            mapped.diagnostics().request_id().map(DiagnosticId::as_str),
            secrets,
        ),
        reference_id: sanitize_diagnostic(
            mapped
                .diagnostics()
                .reference_id()
                .map(DiagnosticId::as_str),
            secrets,
        ),
    }
}

fn rate_limit_record(response: &RawResponse) -> RecordedRateLimit {
    let headers = response.headers();
    RecordedRateLimit {
        limit: headers.limit().map(str::to_owned),
        remaining: headers.remaining().map(str::to_owned),
        reset: headers.reset().map(str::to_owned),
        retry_after: headers.retry_after().map(str::to_owned),
    }
}

fn is_valid_token_bytes(bytes: &[u8]) -> bool {
    !bytes.is_empty()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._~+/=".contains(byte))
}

fn sanitized_token_body(parsed: &Value) -> Value {
    let mut object = Map::new();
    object.insert(
        String::from("access_token"),
        Value::String(REDACTED_TOKEN.to_owned()),
    );
    if let Some(token_type) = parsed.get("token_type").and_then(Value::as_str)
        && token_type == "Bearer"
    {
        object.insert(
            String::from("token_type"),
            Value::String(String::from("Bearer")),
        );
    }
    if let Some(expires_in) = parsed.get("expires_in").and_then(Value::as_u64) {
        object.insert(String::from("expires_in"), Value::from(expires_in));
    }
    Value::Object(object)
}

fn sanitized_error_body(detail: &FailureDetail) -> Value {
    let mut object = Map::new();
    object.insert(
        String::from("errorCategory"),
        Value::String(detail.category.to_owned()),
    );
    if let Some(req) = &detail.request_id {
        object.insert(String::from("requestId"), Value::String(req.clone()));
    }
    if let Some(ref_id) = &detail.reference_id {
        object.insert(String::from("referenceId"), Value::String(ref_id.clone()));
    }
    Value::Object(object)
}

fn parse_json_body(response: &RawResponse) -> Option<Value> {
    serde_json::from_slice(response.body().as_exposed_secret()).ok()
}

fn is_valid_probe_symbol(symbol: &str) -> bool {
    !symbol.is_empty()
        && symbol.len() <= 32
        && symbol
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-')
}

fn select_account_seq(body: &Value) -> Option<SecretValue> {
    let accounts = body.get("result")?.as_array()?;
    let chosen = accounts
        .iter()
        .find(|entry| entry.get("accountType").and_then(Value::as_str) == Some("BROKERAGE"))
        .or_else(|| accounts.first())?;
    let seq = chosen.get("accountSeq")?.as_i64()?;
    Some(SecretValue::new(seq.to_string().into_bytes()))
}

fn select_probe_symbol(body: &Value) -> Option<String> {
    let items = body.get("result")?.get("items")?.as_array()?;
    items
        .iter()
        .filter_map(|entry| entry.get("symbol").and_then(Value::as_str))
        .find(|symbol| is_valid_probe_symbol(symbol))
        .map(str::to_owned)
}

struct ValidatedPayload {
    status: u16,
    summary: ShapeSummary,
    rate_limit: Option<RecordedRateLimit>,
    recorded_body: Option<Value>,
}

struct ReadStepSpec {
    id: &'static str,
    title: &'static str,
    path: &'static str,
    stage: StepStage,
    kind: EndpointKind,
    endpoint: ReadEndpoint,
}

struct RunnerState {
    reports: Vec<StepReport>,
    recordings: Vec<RecordedStep>,
    auth_halted: AuthState,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AuthState {
    Active,
    Halted,
}

struct ReadSession<'a> {
    client: &'a TossHttpClient,
    input: &'a SmokeInput,
    token: &'a SecretValue,
}

impl RunnerState {
    fn new() -> Self {
        Self {
            reports: Vec::new(),
            recordings: Vec::new(),
            auth_halted: AuthState::Active,
        }
    }

    fn push_skipped(&mut self, spec: &ReadStepSpec, reason: &'static str) {
        self.reports.push(StepReport {
            id: spec.id,
            title: spec.title,
            path: spec.path,
            stage: spec.stage,
            status: None,
            outcome: StepOutcome::Skipped(reason),
        });
        self.recordings.push(RecordedStep {
            step: spec.id,
            path: spec.path,
            status: None,
            outcome: "skipped",
            schema_status: "skipped",
            rate_limit: None,
            body: None,
        });
    }

    fn push_transport_failure(&mut self, spec: &ReadStepSpec, error: TossHttpError) {
        self.reports.push(StepReport {
            id: spec.id,
            title: spec.title,
            path: spec.path,
            stage: spec.stage,
            status: None,
            outcome: StepOutcome::TransportFailure(http_error_label(error)),
        });
        self.recordings.push(RecordedStep {
            step: spec.id,
            path: spec.path,
            status: None,
            outcome: "transport_error",
            schema_status: "not_checked",
            rate_limit: None,
            body: None,
        });
    }

    fn push_http_failure(
        &mut self,
        spec: &ReadStepSpec,
        status: u16,
        detail: FailureDetail,
        rate_limit: Option<RecordedRateLimit>,
    ) {
        let err_body = sanitized_error_body(&detail);
        self.reports.push(StepReport {
            id: spec.id,
            title: spec.title,
            path: spec.path,
            stage: spec.stage,
            status: Some(status),
            outcome: StepOutcome::HttpFailure(detail),
        });
        self.recordings.push(RecordedStep {
            step: spec.id,
            path: spec.path,
            status: Some(status),
            outcome: "http_error",
            schema_status: "not_checked",
            rate_limit,
            body: Some(err_body),
        });
    }

    fn push_invalid_json(
        &mut self,
        spec: &ReadStepSpec,
        status: u16,
        rate_limit: Option<RecordedRateLimit>,
    ) {
        let summary = ShapeSummary {
            fields: Vec::new(),
            item_count: None,
            known_enums: std::collections::BTreeMap::new(),
            unknown_enum_count: 0,
            extra_field_count: 0,
            spec_match: SpecMatch::Mismatched(vec![String::from("$: 유효한 JSON이 아님")]),
        };
        self.reports.push(StepReport {
            id: spec.id,
            title: spec.title,
            path: spec.path,
            stage: spec.stage,
            status: Some(status),
            outcome: StepOutcome::SchemaMismatch(summary),
        });
        self.recordings.push(RecordedStep {
            step: spec.id,
            path: spec.path,
            status: Some(status),
            outcome: "schema_mismatch",
            schema_status: "mismatched",
            rate_limit,
            body: None,
        });
    }

    fn push_validated(&mut self, spec: &ReadStepSpec, payload: ValidatedPayload) {
        let (outcome, rec_outcome, schema_status) = match &payload.summary.spec_match {
            SpecMatch::Matched => (StepOutcome::Succeeded(payload.summary), "ok", "matched"),
            SpecMatch::Mismatched(_) => (
                StepOutcome::SchemaMismatch(payload.summary),
                "schema_mismatch",
                "mismatched",
            ),
        };
        self.reports.push(StepReport {
            id: spec.id,
            title: spec.title,
            path: spec.path,
            stage: spec.stage,
            status: Some(payload.status),
            outcome,
        });
        self.recordings.push(RecordedStep {
            step: spec.id,
            path: spec.path,
            status: Some(payload.status),
            outcome: rec_outcome,
            schema_status,
            rate_limit: payload.rate_limit,
            body: payload.recorded_body,
        });
    }

    async fn run_read_step(
        &mut self,
        session: &ReadSession<'_>,
        spec: &ReadStepSpec,
        read_req: &ReadRequest,
    ) -> Option<Value> {
        match self.auth_halted {
            AuthState::Halted => {
                self.push_skipped(spec, "이전 단계 인증 실패(401)로 중단");
                return None;
            }
            AuthState::Active => {}
        }
        let response = match session.client.get(read_req, session.token).await {
            Ok(resp) => resp,
            Err(error) => {
                self.push_transport_failure(spec, error);
                return None;
            }
        };
        let status = response.status();
        let rate_limit = Some(rate_limit_record(&response));
        if status == 401 {
            self.auth_halted = AuthState::Halted;
        }
        if status != 200 {
            let mapped = response.read_error(spec.endpoint, RetryDelay::Unknown);
            let secrets = [
                session.input.client_id(),
                session.input.client_secret(),
                session.token,
            ];
            self.push_http_failure(spec, status, failure_detail(&mapped, &secrets), rate_limit);
            return None;
        }
        let Some(parsed) = parse_json_body(&response) else {
            self.push_invalid_json(spec, status, rate_limit);
            return None;
        };
        let summary = validate_response(spec.kind, &parsed);
        self.push_validated(
            spec,
            ValidatedPayload {
                status,
                summary,
                rate_limit,
                recorded_body: Some(parsed.clone()),
            },
        );
        Some(parsed)
    }

    async fn run_read_discard(
        &mut self,
        session: &ReadSession<'_>,
        spec: &ReadStepSpec,
        read_req: &ReadRequest,
    ) {
        let _discarded = self.run_read_step(session, spec, read_req).await;
    }
}

const TOKEN_STEP_SPEC: ReadStepSpec = ReadStepSpec {
    id: "token",
    title: "토큰 발급",
    path: "/oauth2/token",
    stage: StepStage::Token,
    kind: EndpointKind::Token,
    endpoint: ReadEndpoint::Accounts,
};

async fn run_token_step(
    state: &mut RunnerState,
    client: &TossHttpClient,
    input: &SmokeInput,
) -> Option<SecretValue> {
    let response = match issue_token(client, input.client_id(), input.client_secret()).await {
        Ok(resp) => resp,
        Err(error) => {
            state.push_transport_failure(&TOKEN_STEP_SPEC, error);
            return None;
        }
    };
    let status = response.status();
    let rate_limit = Some(rate_limit_record(&response));
    if status != 200 {
        let mapped = response.token_error(ReadEndpoint::Accounts, RetryDelay::Unknown);
        let secrets = [input.client_id(), input.client_secret()];
        state.push_http_failure(
            &TOKEN_STEP_SPEC,
            status,
            failure_detail(&mapped, &secrets),
            rate_limit,
        );
        return None;
    }
    let Some(parsed) = parse_json_body(&response) else {
        state.push_invalid_json(&TOKEN_STEP_SPEC, status, rate_limit);
        return None;
    };
    let mut summary = validate_response(EndpointKind::Token, &parsed);
    let token_secret = parsed
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|value| is_valid_token_bytes(value.as_bytes()))
        .map(|value| SecretValue::new(value.as_bytes().to_vec()));
    match summary.spec_match {
        SpecMatch::Matched if token_secret.is_none() => {
            summary.spec_match = SpecMatch::Mismatched(vec![String::from(
                "$.access_token: 허용되지 않은 토큰 문자 포함",
            )]);
        }
        SpecMatch::Matched | SpecMatch::Mismatched(_) => {}
    }
    let matched = match summary.spec_match {
        SpecMatch::Matched => true,
        SpecMatch::Mismatched(_) => false,
    };
    let safe_body = sanitized_token_body(&parsed);
    state.push_validated(
        &TOKEN_STEP_SPEC,
        ValidatedPayload {
            status,
            summary,
            rate_limit,
            recorded_body: Some(safe_body),
        },
    );
    if matched { token_secret } else { None }
}

fn step_failed(report: &StepReport) -> bool {
    match report.outcome {
        StepOutcome::Succeeded(_) | StepOutcome::Skipped(_) => false,
        StepOutcome::SchemaMismatch(_)
        | StepOutcome::HttpFailure(_)
        | StepOutcome::TransportFailure(_) => true,
    }
}

fn determine_exit_code(reports: &[StepReport]) -> SmokeExitCode {
    let mut token_failed = false;
    let mut accounts_failed = false;
    let mut holdings_failed = false;
    let mut additional_failed = false;
    for item in reports {
        if step_failed(item) {
            match item.stage {
                StepStage::Token => token_failed = true,
                StepStage::Accounts => accounts_failed = true,
                StepStage::Holdings => holdings_failed = true,
                StepStage::Additional => additional_failed = true,
            }
        }
    }
    if token_failed {
        return SmokeExitCode::TokenStepFailed;
    }
    if accounts_failed {
        return SmokeExitCode::AccountsStepFailed;
    }
    if holdings_failed {
        return SmokeExitCode::HoldingsStepFailed;
    }
    if additional_failed {
        return SmokeExitCode::AdditionalStepFailed;
    }
    SmokeExitCode::Success
}

fn account_step_specs() -> [ReadStepSpec; 4] {
    [
        ReadStepSpec {
            id: "holdings",
            title: "보유 주식 조회",
            path: "/api/v1/holdings",
            stage: StepStage::Holdings,
            kind: EndpointKind::Holdings,
            endpoint: ReadEndpoint::Holdings,
        },
        ReadStepSpec {
            id: "buying-power-krw",
            title: "매수가능금액(KRW)",
            path: "/api/v1/buying-power",
            stage: StepStage::Additional,
            kind: EndpointKind::BuyingPower,
            endpoint: ReadEndpoint::BuyingPower,
        },
        ReadStepSpec {
            id: "buying-power-usd",
            title: "매수가능금액(USD)",
            path: "/api/v1/buying-power",
            stage: StepStage::Additional,
            kind: EndpointKind::BuyingPower,
            endpoint: ReadEndpoint::BuyingPower,
        },
        ReadStepSpec {
            id: "transactions",
            title: "거래내역 조회",
            path: "/api/v1/transactions",
            stage: StepStage::Additional,
            kind: EndpointKind::Transactions,
            endpoint: ReadEndpoint::Transactions,
        },
    ]
}

async fn run_account_scoped_steps(
    state: &mut RunnerState,
    session: &ReadSession<'_>,
    account: Option<&SecretValue>,
) -> String {
    let [holdings_spec, bp_krw_spec, bp_usd_spec, tx_spec] = account_step_specs();
    let Some(account_header) = account else {
        for spec in [&holdings_spec, &bp_krw_spec, &bp_usd_spec, &tx_spec] {
            state.push_skipped(spec, "계좌 없음으로 건너뜀");
        }
        return String::from(DEFAULT_PROBE_SYMBOL);
    };
    let clone_account = || SecretValue::new(account_header.as_exposed_secret().to_vec());
    let holdings_req = ReadRequest::new(ReadRoute::Holdings).with_account(clone_account());
    let holdings_body = state
        .run_read_step(session, &holdings_spec, &holdings_req)
        .await;
    let probe_symbol = holdings_body
        .as_ref()
        .and_then(select_probe_symbol)
        .unwrap_or_else(|| String::from(DEFAULT_PROBE_SYMBOL));
    let bp_krw_req = ReadRequest::new(ReadRoute::BuyingPower)
        .with_account(clone_account())
        .with_query(QueryKey::Currency, String::from("KRW"));
    state
        .run_read_discard(session, &bp_krw_spec, &bp_krw_req)
        .await;
    let bp_usd_req = ReadRequest::new(ReadRoute::BuyingPower)
        .with_account(clone_account())
        .with_query(QueryKey::Currency, String::from("USD"));
    state
        .run_read_discard(session, &bp_usd_spec, &bp_usd_req)
        .await;
    let tx_req = ReadRequest::new(ReadRoute::Transactions)
        .with_account(clone_account())
        .with_query(QueryKey::Limit, String::from("5"));
    state.run_read_discard(session, &tx_spec, &tx_req).await;
    probe_symbol
}

const MARKET_STEP_SPECS: [ReadStepSpec; 7] = [
    ReadStepSpec {
        id: "exchange-rate",
        title: "환율 조회(USD/KRW)",
        path: "/api/v1/exchange-rate",
        stage: StepStage::Additional,
        kind: EndpointKind::ExchangeRate,
        endpoint: ReadEndpoint::FxRate,
    },
    ReadStepSpec {
        id: "market-calendar-kr",
        title: "국내 장 운영 정보",
        path: "/api/v1/market-calendar/KR",
        stage: StepStage::Additional,
        kind: EndpointKind::CalendarKr,
        endpoint: ReadEndpoint::Calendar,
    },
    ReadStepSpec {
        id: "market-calendar-us",
        title: "미국 장 운영 정보",
        path: "/api/v1/market-calendar/US",
        stage: StepStage::Additional,
        kind: EndpointKind::CalendarUs,
        endpoint: ReadEndpoint::Calendar,
    },
    ReadStepSpec {
        id: "market-indicators",
        title: "시장 지수 시세",
        path: "/api/v1/market-indicators/prices",
        stage: StepStage::Additional,
        kind: EndpointKind::IndicatorQuotes,
        endpoint: ReadEndpoint::IndicatorQuotes,
    },
    ReadStepSpec {
        id: "sectors",
        title: "섹터 목록",
        path: "/api/v1/sectors",
        stage: StepStage::Additional,
        kind: EndpointKind::Sectors,
        endpoint: ReadEndpoint::Sectors,
    },
    ReadStepSpec {
        id: "prices",
        title: "현재가 조회",
        path: "/api/v1/prices",
        stage: StepStage::Additional,
        kind: EndpointKind::Quotes,
        endpoint: ReadEndpoint::Quotes,
    },
    ReadStepSpec {
        id: "candles",
        title: "일봉 조회",
        path: "/api/v1/candles",
        stage: StepStage::Additional,
        kind: EndpointKind::Candles,
        endpoint: ReadEndpoint::Candles,
    },
];

fn market_requests(probe_symbol: &str) -> [ReadRequest; 7] {
    [
        ReadRequest::new(ReadRoute::FxRate)
            .with_query(QueryKey::BaseCurrency, String::from("USD"))
            .with_query(QueryKey::QuoteCurrency, String::from("KRW")),
        ReadRequest::new(ReadRoute::CalendarKr),
        ReadRequest::new(ReadRoute::CalendarUs),
        ReadRequest::new(ReadRoute::IndicatorQuotes)
            .with_query(QueryKey::Symbols, String::from("KOSPI,KOSDAQ")),
        ReadRequest::new(ReadRoute::Sectors),
        ReadRequest::new(ReadRoute::Quotes).with_query(QueryKey::Symbols, probe_symbol.to_owned()),
        ReadRequest::new(ReadRoute::Candles)
            .with_query(QueryKey::Symbol, probe_symbol.to_owned())
            .with_query(QueryKey::Interval, String::from("1d"))
            .with_query(QueryKey::Count, String::from("5"))
            .with_query(QueryKey::Adjusted, String::from("true")),
    ]
}

async fn run_market_steps(state: &mut RunnerState, session: &ReadSession<'_>, probe_symbol: &str) {
    let requests = market_requests(probe_symbol);
    for (spec, read_req) in MARKET_STEP_SPECS.iter().zip(requests.iter()) {
        state.run_read_discard(session, spec, read_req).await;
    }
}

pub async fn run_all(client: &TossHttpClient, input: &SmokeInput) -> RunResult {
    let mut state = RunnerState::new();
    let Some(token) = run_token_step(&mut state, client, input).await else {
        let exit_code = determine_exit_code(&state.reports);
        return RunResult {
            reports: state.reports,
            recordings: state.recordings,
            exit_code,
            access_token: None,
        };
    };
    let session = ReadSession {
        client,
        input,
        token: &token,
    };
    let accounts_spec = ReadStepSpec {
        id: "accounts",
        title: "계좌 목록 조회",
        path: "/api/v1/accounts",
        stage: StepStage::Accounts,
        kind: EndpointKind::Accounts,
        endpoint: ReadEndpoint::Accounts,
    };
    let accounts_req = ReadRequest::new(ReadRoute::Accounts);
    let accounts_body = state
        .run_read_step(&session, &accounts_spec, &accounts_req)
        .await;
    let accounts_ok = state
        .reports
        .last()
        .is_some_and(|report| !step_failed(report));
    if !accounts_ok {
        let exit_code = determine_exit_code(&state.reports);
        return RunResult {
            reports: state.reports,
            recordings: state.recordings,
            exit_code,
            access_token: Some(token),
        };
    }
    let account_header = accounts_body.as_ref().and_then(select_account_seq);
    let probe_symbol =
        run_account_scoped_steps(&mut state, &session, account_header.as_ref()).await;
    run_market_steps(&mut state, &session, &probe_symbol).await;
    let exit_code = determine_exit_code(&state.reports);
    RunResult {
        reports: state.reports,
        recordings: state.recordings,
        exit_code,
        access_token: Some(token),
    }
}
