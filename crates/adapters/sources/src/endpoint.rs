use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};

use application::CredentialSource;
use reqwest::Url;

use crate::{CredentialRequirement, HostAllowance, SourceCredential, SourceHttpError, SourceSpec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EndpointMode {
    Live,
    Loopback(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceEndpoint {
    mode: EndpointMode,
}

impl SourceEndpoint {
    #[must_use]
    pub const fn live() -> Self {
        Self {
            mode: EndpointMode::Live,
        }
    }

    pub const fn local(port: u16) -> Result<Self, SourceHttpError> {
        if port == 0 {
            return Err(SourceHttpError::InvalidEndpoint);
        }
        Ok(Self {
            mode: EndpointMode::Loopback(port),
        })
    }

    pub(crate) const fn mode(self) -> EndpointMode {
        self.mode
    }
}

fn raw_authority_and_path(raw: &str) -> Result<(&str, &str), SourceHttpError> {
    if raw.is_empty()
        || raw.len() > 4096
        || !raw.bytes().all(|byte| (0x21..=0x7E).contains(&byte))
        || raw.contains('\\')
    {
        return Err(SourceHttpError::DisallowedUrl);
    }
    let Some((_scheme, rest)) = raw.split_once("://") else {
        return Err(SourceHttpError::DisallowedUrl);
    };
    let before_query = rest.split_once(['?', '#']).map_or(rest, |(head, _)| head);
    let (authority, path) = before_query
        .split_once('/')
        .map_or((before_query, ""), |(host, tail)| (host, tail));
    if authority.is_empty()
        || authority.contains(':')
        || authority.contains('%')
        || authority.ends_with('.')
    {
        return Err(SourceHttpError::DisallowedUrl);
    }
    Ok((authority, path))
}

fn has_unsafe_path_or_query(raw_path: &str, url: &Url) -> bool {
    let lower_raw_path = raw_path.to_ascii_lowercase();
    let bad_encoding = ["%00", "%5c", "%2f", "%0d", "%0a", "%252e", "%252f", "%255c"]
        .iter()
        .any(|needle| lower_raw_path.contains(needle));
    let dot_segment = raw_path.split('/').any(|segment| {
        let decoded = segment.to_ascii_lowercase().replace("%2e", ".");
        decoded == "." || decoded == ".."
    });
    let bad_query = url.query().is_some_and(|query| {
        let lower = query.to_ascii_lowercase();
        lower.contains("%0d") || lower.contains("%0a") || lower.contains("%00")
    });
    bad_encoding || dot_segment || bad_query
}

fn validate_secret_query_value(value: &[u8]) -> Result<&str, SourceHttpError> {
    if value.is_empty() || value.len() > 256 {
        return Err(SourceHttpError::InvalidCredential);
    }
    let text = std::str::from_utf8(value).map_err(|_error| SourceHttpError::InvalidCredential)?;
    let safe = text
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || b"-._~".contains(&byte));
    if !safe {
        return Err(SourceHttpError::InvalidCredential);
    }
    Ok(text)
}

fn credential_param_name(source: CredentialSource) -> &'static str {
    match source {
        CredentialSource::Dart => "crtfc_key",
        CredentialSource::Fred => "api_key",
    }
}

fn validate_credential<'secret>(
    spec: &SourceSpec,
    credential: Option<&'secret SourceCredential>,
) -> Result<Option<(&'static str, &'secret str)>, SourceHttpError> {
    match (spec.credential(), credential) {
        (CredentialRequirement::None | CredentialRequirement::Optional(_), None) => Ok(None),
        (CredentialRequirement::None, Some(_)) => Err(SourceHttpError::InvalidCredential),
        (CredentialRequirement::Optional(expected), Some(provided)) => {
            if provided.source() != expected {
                return Err(SourceHttpError::InvalidCredential);
            }
            let text = validate_secret_query_value(provided.value().as_exposed_secret())?;
            Ok(Some((credential_param_name(expected), text)))
        }
    }
}

fn percent_decode_ascii_lowercase(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut idx = 0_usize;
    while let Some(&byte) = bytes.get(idx) {
        if byte == b'%'
            && let (Some(&hi), Some(&lo)) = (
                bytes.get(idx.saturating_add(1_usize)),
                bytes.get(idx.saturating_add(2_usize)),
            )
            && let (Some(high), Some(low)) = (
                char::from(hi).to_digit(16_u32),
                char::from(lo).to_digit(16_u32),
            )
            && let Ok(decoded) = u8::try_from((high << 4_u32) | low)
        {
            out.push(decoded.to_ascii_lowercase());
            idx = idx.saturating_add(3_usize);
        } else {
            out.push(byte.to_ascii_lowercase());
            idx = idx.saturating_add(1_usize);
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub(crate) fn resolve_wire_url(
    spec: &SourceSpec,
    endpoint: SourceEndpoint,
    raw_url: &str,
    credential: Option<&SourceCredential>,
) -> Result<Url, SourceHttpError> {
    let secret_param = validate_credential(spec, credential)?;
    let (authority, raw_path) = raw_authority_and_path(raw_url)?;
    let mut parsed = Url::parse(raw_url).map_err(|_error| SourceHttpError::DisallowedUrl)?;
    if parsed.scheme() != "https"
        || parsed.fragment().is_some()
        || has_unsafe_path_or_query(raw_path, &parsed)
    {
        return Err(SourceHttpError::DisallowedUrl);
    }
    let host_ok = match parsed.host_str() {
        Some(host) => {
            host.eq_ignore_ascii_case(authority)
                && host.parse::<IpAddr>().is_err()
                && spec.host_allowance(host) == HostAllowance::Allowed
        }
        None => false,
    };
    if !host_ok {
        return Err(SourceHttpError::DisallowedUrl);
    }
    let forbidden_secret_param = parsed.query().is_some_and(|query| {
        query.split(['&', ';']).any(|pair| {
            let key = pair.split_once('=').map_or(pair, |(name, _)| name);
            let normalized = percent_decode_ascii_lowercase(key);
            normalized == "crtfc_key" || normalized == "api_key"
        })
    });
    if forbidden_secret_param {
        return Err(SourceHttpError::DisallowedUrl);
    }
    if let Some((key_name, key_value)) = secret_param {
        parsed.query_pairs_mut().append_pair(key_name, key_value);
    }
    match endpoint.mode() {
        EndpointMode::Live => Ok(parsed),
        EndpointMode::Loopback(port) => {
            let address = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
            let mut local = Url::parse(&format!("http://{address}"))
                .map_err(|_error| SourceHttpError::InvalidEndpoint)?;
            local.set_path(parsed.path());
            local.set_query(parsed.query());
            Ok(local)
        }
    }
}
