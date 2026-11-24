use crate::account_read::retry_delay;
use crate::{PublicDocsRoute, RawResponse, TossHttpClient, TossHttpError, sha256_hex};
use application::{
    HttpLastModified, SpecDocumentSnapshot, SpecFetchError, SpecHash, SpecVersionLabel,
    TossSpecBaseline, TossSpecObservation, TossSpecSource,
};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub const PINNED_SPEC_VERSION: &str = "1.2.28";
pub const PINNED_LAST_MODIFIED: &str = "Wed, 04 Nov 2026 01:48:38 GMT";
pub const PINNED_NORMALIZED_SPEC_SHA256: &str =
    "e722dad95f193da7b0917193e3e034cfe547bbc03d266fcb00fc50af26baa42b";

fn canonical_value(value: &Value) -> Value {
    match value {
        Value::Object(entries) => {
            let sorted: BTreeMap<&String, Value> = entries
                .iter()
                .map(|(key, item)| (key, canonical_value(item)))
                .collect();
            let mut normalized = Map::with_capacity(sorted.len());
            for (key, item) in sorted {
                normalized.insert(key.clone(), item);
            }
            Value::Object(normalized)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonical_value).collect()),
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => value.clone(),
    }
}

pub fn normalized_json_sha256(bytes: &[u8]) -> Result<SpecHash, SpecFetchError> {
    let parsed: Value =
        serde_json::from_slice(bytes).map_err(|_error| SpecFetchError::InvalidResponse)?;
    let normalized = canonical_value(&parsed);
    let encoded =
        serde_json::to_vec(&normalized).map_err(|_error| SpecFetchError::InvalidResponse)?;
    let hex = sha256_hex(&encoded);
    SpecHash::try_new(&hex).map_err(|_error| SpecFetchError::InvalidResponse)
}

fn extract_spec_version(bytes: &[u8]) -> Option<SpecVersionLabel> {
    let parsed: Value = serde_json::from_slice(bytes).ok()?;
    let label = parsed.get("info")?.get("version")?.as_str()?;
    SpecVersionLabel::try_new(label).ok()
}

fn extract_versions_current(bytes: &[u8]) -> Option<SpecVersionLabel> {
    let parsed: Value = serde_json::from_slice(bytes).ok()?;
    for key in ["current", "latest", "version"] {
        if let Some(label) = parsed.get(key).and_then(Value::as_str)
            && let Ok(parsed_label) = SpecVersionLabel::try_new(label)
        {
            return Some(parsed_label);
        }
    }
    None
}

fn parse_last_modified(response: &RawResponse) -> Result<Option<HttpLastModified>, SpecFetchError> {
    match response.headers().last_modified() {
        None => Ok(None),
        Some(raw) => HttpLastModified::try_new(raw)
            .map(Some)
            .map_err(|_error| SpecFetchError::InvalidResponse),
    }
}

fn map_http_error(error: TossHttpError) -> SpecFetchError {
    match error {
        TossHttpError::ResponseTooLarge => SpecFetchError::ResponseTooLarge,
        TossHttpError::RateLimited => SpecFetchError::RateLimited(application::RetryDelay::Unknown),
        TossHttpError::InvalidCredential => SpecFetchError::AccessDenied,
        TossHttpError::InvalidEndpoint
        | TossHttpError::Configuration
        | TossHttpError::Transport => SpecFetchError::Network,
    }
}

fn check_status(response: &RawResponse) -> Result<(), SpecFetchError> {
    let status = response.status();
    if (200..300).contains(&status) {
        return Ok(());
    }
    if status == 429 {
        return Err(SpecFetchError::RateLimited(retry_delay(
            response.headers().retry_after(),
        )));
    }
    if status == 401 || status == 403 {
        return Err(SpecFetchError::AccessDenied);
    }
    if (500..600).contains(&status) {
        return Err(SpecFetchError::ServerError);
    }
    Err(SpecFetchError::InvalidResponse)
}

pub fn pinned_spec_baseline() -> Result<TossSpecBaseline, SpecFetchError> {
    let raw_hash = SpecHash::try_new(broker_toss_schema::SPEC_SHA256)
        .map_err(|_error| SpecFetchError::InvalidResponse)?;
    let normalized_hash = SpecHash::try_new(PINNED_NORMALIZED_SPEC_SHA256)
        .map_err(|_error| SpecFetchError::InvalidResponse)?;
    let version = SpecVersionLabel::try_new(PINNED_SPEC_VERSION)
        .map_err(|_error| SpecFetchError::InvalidResponse)?;
    let last_modified = HttpLastModified::try_new(PINNED_LAST_MODIFIED)
        .map_err(|_error| SpecFetchError::InvalidResponse)?;
    Ok(TossSpecBaseline::new(raw_hash, normalized_hash, version)
        .with_spec_last_modified(last_modified))
}

#[derive(Debug)]
pub struct TossSpecClient {
    http: TossHttpClient,
}

impl TossSpecClient {
    #[must_use]
    pub const fn new(http: TossHttpClient) -> Self {
        Self { http }
    }
}

impl TossSpecSource for TossSpecClient {
    async fn fetch(&self) -> Result<TossSpecObservation, SpecFetchError> {
        let versions_response = self
            .http
            .get_public_doc(PublicDocsRoute::Versions)
            .await
            .map_err(map_http_error)?;
        check_status(&versions_response)?;
        let versions_bytes = versions_response.body().as_exposed_secret();
        let versions_hash = normalized_json_sha256(versions_bytes)?;
        let versions_current = extract_versions_current(versions_bytes);
        let versions_last_modified = parse_last_modified(&versions_response)?;

        let spec_response = self
            .http
            .get_public_doc(PublicDocsRoute::LatestOpenApi)
            .await
            .map_err(map_http_error)?;
        check_status(&spec_response)?;
        let spec_bytes = spec_response.body().as_exposed_secret();
        let normalized_spec_hash = normalized_json_sha256(spec_bytes)?;
        let spec_version = extract_spec_version(spec_bytes);
        let spec_last_modified = parse_last_modified(&spec_response)?;

        Ok(TossSpecObservation::new(
            SpecDocumentSnapshot::new(normalized_spec_hash, spec_version, spec_last_modified),
            SpecDocumentSnapshot::new(versions_hash, versions_current, versions_last_modified),
        ))
    }
}
