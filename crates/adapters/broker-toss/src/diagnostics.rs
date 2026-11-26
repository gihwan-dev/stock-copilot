use application::{
    BrokerCall, CallMethod, CallResult, DiagnosticEvent, DiagnosticText, DiagnosticsSink,
    HttpStatusCode, ReadEndpoint, ResponseMismatch, TransportFailure,
};
use domain::Timestamp;
use std::sync::{Arc, OnceLock};

use crate::{ApiErrorCode, OAuthErrorCode, RawResponse, TossErrorBody, TossHttpError};
use application::Clock;

const VALUE: &str = "<value>";

static SINK: OnceLock<Arc<dyn DiagnosticsSink>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticsInstall {
    Installed,
    AlreadyInstalled,
}

pub fn install_diagnostics(sink: Arc<dyn DiagnosticsSink>) -> DiagnosticsInstall {
    match SINK.set(sink) {
        Ok(()) => DiagnosticsInstall::Installed,
        Err(_rejected) => DiagnosticsInstall::AlreadyInstalled,
    }
}

pub fn record_diagnostic(event: &DiagnosticEvent) {
    if let Some(sink) = SINK.get() {
        sink.record(event);
    }
}

struct CallRecord<'a> {
    method: CallMethod,
    path: &'a str,
    started: Timestamp,
    finished: Timestamp,
}

pub(crate) type CallStart<'a> = (CallMethod, &'a str, Timestamp);

fn finish<'a>(clock: &dyn Clock, start: CallStart<'a>) -> CallRecord<'a> {
    let (method, path, started) = start;
    CallRecord {
        method,
        path,
        started,
        finished: clock.now(),
    }
}

pub(crate) async fn observed_send(
    clock: &dyn Clock,
    start: CallStart<'_>,
    pending: impl Future<Output = Result<reqwest::Response, reqwest::Error>>,
) -> Result<reqwest::Response, TossHttpError> {
    match pending.await {
        Ok(response) => Ok(response),
        Err(error) => {
            record_transport(&finish(clock, start), transport_failure(&error));
            Err(TossHttpError::Transport)
        }
    }
}

pub(crate) async fn observed_receive(
    clock: &dyn Clock,
    start: CallStart<'_>,
    transport: reqwest::Response,
) -> Result<RawResponse, TossHttpError> {
    let received = crate::http::receive(transport).await;
    let call = finish(clock, start);
    match &received {
        Ok(response) if (200..300).contains(&response.status()) => {
            record_status(&call, response.status(), None);
        }
        Ok(response) => {
            record_status(&call, response.status(), Some(&response.error_body()));
        }
        Err(TossHttpError::ResponseTooLarge) => {
            record_transport(&call, TransportFailure::TooLarge);
        }
        Err(
            TossHttpError::Transport
            | TossHttpError::InvalidEndpoint
            | TossHttpError::InvalidCredential
            | TossHttpError::Configuration
            | TossHttpError::RateLimited,
        ) => record_transport(&call, TransportFailure::Body),
    }
    received
}

fn transport_failure(error: &reqwest::Error) -> TransportFailure {
    if error.is_timeout() {
        TransportFailure::Timeout
    } else if error.is_connect() {
        TransportFailure::Connect
    } else if error.is_body() || error.is_decode() {
        TransportFailure::Body
    } else {
        TransportFailure::Other
    }
}

fn record_transport(call: &CallRecord<'_>, failure: TransportFailure) {
    record_call(call, CallResult::Transport(failure), None);
}

fn record_status(call: &CallRecord<'_>, status: u16, body: Option<&TossErrorBody>) {
    record_call(call, CallResult::Status(HttpStatusCode::new(status)), body);
}

fn record_call(call: &CallRecord<'_>, result: CallResult, body: Option<&TossErrorBody>) {
    let (code, request) = body.map_or((None, None), error_identity);
    record_diagnostic(&DiagnosticEvent::BrokerCall(BrokerCall {
        method: call.method,
        path: DiagnosticText::new(call.path),
        result,
        code,
        request,
        duration: call.started.elapsed_until(call.finished).ok(),
    }));
}

fn error_identity(body: &TossErrorBody) -> (Option<DiagnosticText>, Option<DiagnosticText>) {
    match body {
        TossErrorBody::Missing => (None, None),
        TossErrorBody::Malformed { diagnostics } => (
            Some(DiagnosticText::new("malformed")),
            diagnostics
                .request_id()
                .map(|id| DiagnosticText::new(id.as_str())),
        ),
        TossErrorBody::Api {
            code, diagnostics, ..
        } => (
            Some(DiagnosticText::new(api_code_name(*code))),
            diagnostics
                .request_id()
                .map(|id| DiagnosticText::new(id.as_str())),
        ),
        TossErrorBody::OAuth { code, diagnostics } => (
            Some(DiagnosticText::new(oauth_code_name(*code))),
            diagnostics
                .request_id()
                .map(|id| DiagnosticText::new(id.as_str())),
        ),
    }
}

fn api_code_name(code: ApiErrorCode) -> &'static str {
    ApiErrorCode::known()
        .iter()
        .find_map(|(name, known)| (*known == code).then_some(*name))
        .unwrap_or("unknown")
}

fn oauth_code_name(code: OAuthErrorCode) -> &'static str {
    OAuthErrorCode::known()
        .iter()
        .find_map(|(name, known)| (*known == code).then_some(*name))
        .unwrap_or("unknown")
}

pub(crate) fn decode_failed(endpoint: ReadEndpoint, error: &serde_json::Error) {
    record_diagnostic(&DiagnosticEvent::ResponseRejected {
        endpoint,
        mismatch: ResponseMismatch::Schema,
        detail: DiagnosticText::new(&serde_detail(error)),
    });
}

pub(crate) fn validation_failed(endpoint: ReadEndpoint, stage: &str) {
    record_diagnostic(&DiagnosticEvent::ResponseRejected {
        endpoint,
        mismatch: ResponseMismatch::Validation,
        detail: DiagnosticText::new(stage),
    });
}

pub(crate) fn staged<T>(
    endpoint: ReadEndpoint,
    stage: &str,
    result: Result<T, application::PortError>,
) -> Result<T, application::PortError> {
    result.inspect_err(|error| {
        let mismatch = match error.kind() {
            application::PortErrorKind::InvalidResponse(mismatch) => mismatch,
            application::PortErrorKind::MasterNotReady(_)
            | application::PortErrorKind::AccessDenied(_)
            | application::PortErrorKind::Unavailable { .. }
            | application::PortErrorKind::NotFound(_)
            | application::PortErrorKind::Unsupported(_)
            | application::PortErrorKind::InvalidRequest(_) => ResponseMismatch::Validation,
        };
        record_diagnostic(&DiagnosticEvent::ResponseRejected {
            endpoint,
            mismatch,
            detail: DiagnosticText::new(stage),
        });
    })
}

pub(crate) fn schema_failed(endpoint: ReadEndpoint, stage: &str) {
    record_diagnostic(&DiagnosticEvent::ResponseRejected {
        endpoint,
        mismatch: ResponseMismatch::Schema,
        detail: DiagnosticText::new(stage),
    });
}

pub(crate) fn serde_detail(error: &serde_json::Error) -> String {
    let category = match error.classify() {
        serde_json::error::Category::Io => "io",
        serde_json::error::Category::Syntax => "syntax",
        serde_json::error::Category::Data => "data",
        serde_json::error::Category::Eof => "eof",
    };
    let message = error.to_string();
    let body = message
        .rsplit_once(" at line ")
        .map_or(message.as_str(), |(head, _position)| head);
    let position = match (error.line(), error.column()) {
        (0, _) => String::new(),
        (line, column) => format!(" at line {line} column {column}"),
    };
    format!("{category}: {}{position}", strip_values(body))
}

pub(crate) fn strip_values(message: &str) -> String {
    for prefix in ["invalid type: ", "invalid value: "] {
        if let Some(rest) = message.strip_prefix(prefix) {
            return match rest.split_once(", expected ") {
                Some((unexpected, expected)) => format!(
                    "{prefix}{}, expected {}",
                    value_kind(unexpected),
                    mask_spans(expected)
                ),
                None => format!("{prefix}{}", value_kind(rest)),
            };
        }
    }
    if let Some(rest) = message.strip_prefix("unknown variant ") {
        return match rest.split_once(", expected ") {
            Some((_variant, expected)) => {
                format!(
                    "unknown variant {VALUE}, expected {}",
                    mask_quoted(expected)
                )
            }
            None => format!("unknown variant {VALUE}"),
        };
    }
    for prefix in [
        "missing field ",
        "unknown field ",
        "duplicate field ",
        "invalid length ",
    ] {
        if message.starts_with(prefix) {
            return mask_quoted(message);
        }
    }
    mask_digits(&mask_spans(message))
}

fn value_kind(unexpected: &str) -> String {
    let kind = unexpected
        .split_once(['`', '"'])
        .map_or(unexpected, |(head, _value)| head)
        .trim_end();
    if kind.len() == unexpected.len() {
        kind.to_owned()
    } else {
        format!("{kind} {VALUE}")
    }
}

fn mask_spans(text: &str) -> String {
    mask_between(&mask_quoted(text), '`')
}

fn mask_quoted(text: &str) -> String {
    mask_between(text, '"')
}

fn mask_between(text: &str, delimiter: char) -> String {
    let mut output = String::new();
    let mut inside = false;
    for character in text.chars() {
        if character == delimiter {
            if !inside {
                output.push_str(VALUE);
            }
            inside = !inside;
        } else if !inside {
            output.push(character);
        }
    }
    output
}

fn mask_digits(text: &str) -> String {
    let mut output = String::new();
    let mut previous_digit = false;
    for character in text.chars() {
        if character.is_ascii_digit() {
            if !previous_digit {
                output.push('#');
            }
            previous_digit = true;
        } else {
            output.push(character);
            previous_digit = false;
        }
    }
    output
}

#[cfg(test)]
mod tests;
