use std::fmt;

use crate::NormalizedUrl;

#[derive(Clone, PartialEq, Eq)]
pub struct ExternalUrlInput(String);

impl ExternalUrlInput {
    #[must_use]
    pub fn new(value: &str) -> Self {
        Self(value.to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ExternalUrlInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExternalUrlInput")
            .field("byte_len", &self.0.len())
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalUrlScheme {
    Https,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalUrlPort {
    ImplicitHttps,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalUrlRejection {
    Empty,
    TooLong,
    ForbiddenCharacter,
    NonHttpsScheme,
    UserInfoNotAllowed,
    ExplicitPortNotAllowed,
    TrailingDotNotAllowed,
    PunycodeNotAllowed,
    InvalidHost,
    FragmentNotAllowed,
    InvalidPercentEncoding,
    InvalidPath,
    InvalidQuery,
    OpenRedirectNotAllowed,
    NotAllowed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalUrlHost(String);

impl ExternalUrlHost {
    pub fn try_new(value: &str) -> Result<Self, ExternalUrlRejection> {
        if value.contains('@') {
            return Err(ExternalUrlRejection::UserInfoNotAllowed);
        }
        if value.contains(':') {
            return Err(ExternalUrlRejection::ExplicitPortNotAllowed);
        }
        if value.ends_with('.') {
            return Err(ExternalUrlRejection::TrailingDotNotAllowed);
        }
        if value
            .split('.')
            .any(|label| label.to_ascii_lowercase().starts_with("xn--"))
        {
            return Err(ExternalUrlRejection::PunycodeNotAllowed);
        }
        let Some((_, tld)) = value.rsplit_once('.') else {
            return Err(ExternalUrlRejection::InvalidHost);
        };
        if value.len() > 253
            || tld.len() < 2
            || !tld.bytes().all(|byte| byte.is_ascii_lowercase())
            || tld == "localhost"
            || tld == "local"
            || value.split('.').any(|label| {
                label.is_empty()
                    || label.len() > 63
                    || label.starts_with('-')
                    || label.ends_with('-')
                    || !label.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
                    })
            })
        {
            return Err(ExternalUrlRejection::InvalidHost);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug)]
struct HexNibble(u8);

impl HexNibble {
    fn try_new(byte: u8) -> Option<Self> {
        match byte {
            b'0'..=b'9' => Some(Self(byte.saturating_sub(b'0'))),
            b'A'..=b'F' => Some(Self(byte.saturating_sub(b'A').saturating_add(10))),
            b'a'..=b'f' => Some(Self(byte.saturating_sub(b'a').saturating_add(10))),
            _ => None,
        }
    }

    const fn as_u8(&self) -> u8 {
        self.0
    }
}

#[derive(Debug)]
struct PercentDecodedPath(String);

impl PercentDecodedPath {
    fn try_new(value: &str) -> Result<Self, ExternalUrlRejection> {
        let mut decoded_once = Vec::with_capacity(value.len());
        let mut bytes = value.bytes();
        while let Some(byte) = bytes.next() {
            if byte != b'%' {
                decoded_once.push(byte);
                continue;
            }
            let (Some(hi_byte), Some(lo_byte)) = (bytes.next(), bytes.next()) else {
                return Err(ExternalUrlRejection::InvalidPercentEncoding);
            };
            let (Some(hi), Some(lo)) = (HexNibble::try_new(hi_byte), HexNibble::try_new(lo_byte))
            else {
                return Err(ExternalUrlRejection::InvalidPercentEncoding);
            };
            let decoded = hi.as_u8().saturating_mul(16).saturating_add(lo.as_u8());
            if decoded < 0x20
                || decoded == 0x7F
                || decoded == b'/'
                || decoded == b'\\'
                || hi_byte.is_ascii_lowercase()
                || lo_byte.is_ascii_lowercase()
            {
                return Err(ExternalUrlRejection::InvalidPercentEncoding);
            }
            decoded_once.push(decoded);
        }
        let mut second = decoded_once.iter().copied();
        while let Some(byte) = second.next() {
            if byte != b'%' {
                continue;
            }
            let (Some(hi_byte), Some(lo_byte)) = (second.next(), second.next()) else {
                continue;
            };
            let (Some(hi), Some(lo)) = (HexNibble::try_new(hi_byte), HexNibble::try_new(lo_byte))
            else {
                continue;
            };
            let nested = hi.as_u8().saturating_mul(16).saturating_add(lo.as_u8());
            if nested < 0x20
                || nested == 0x7F
                || nested == b'/'
                || nested == b'\\'
                || nested == b'.'
            {
                return Err(ExternalUrlRejection::InvalidPercentEncoding);
            }
        }
        Ok(Self(String::from_utf8_lossy(&decoded_once).into_owned()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalUrlPath(String);

impl ExternalUrlPath {
    pub fn try_new(value: &str) -> Result<Self, ExternalUrlRejection> {
        if !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@/%".contains(&byte))
        {
            return Err(ExternalUrlRejection::ForbiddenCharacter);
        }
        if !value.starts_with('/') || value.contains("//") {
            return Err(ExternalUrlRejection::InvalidPath);
        }
        let decoded = PercentDecodedPath::try_new(value)?;
        let forbidden_schemes = [
            "http:",
            "https:",
            "javascript:",
            "data:",
            "file:",
            "vbscript:",
            "blob:",
            "about:",
        ];
        if decoded.0.split('/').skip(1).any(|segment| {
            let lowered = segment.trim_start().to_ascii_lowercase();
            lowered == "."
                || lowered == ".."
                || lowered.contains("://")
                || forbidden_schemes
                    .iter()
                    .any(|prefix| lowered.starts_with(prefix))
        }) {
            return Err(ExternalUrlRejection::InvalidPath);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug)]
struct QueryPercentValidated(String);

impl QueryPercentValidated {
    fn try_new(value: &str) -> Result<Self, ExternalUrlRejection> {
        let mut bytes = value.bytes();
        while let Some(byte) = bytes.next() {
            if byte != b'%' {
                continue;
            }
            let (Some(hi_byte), Some(lo_byte)) = (bytes.next(), bytes.next()) else {
                return Err(ExternalUrlRejection::InvalidPercentEncoding);
            };
            let (Some(hi), Some(lo)) = (HexNibble::try_new(hi_byte), HexNibble::try_new(lo_byte))
            else {
                return Err(ExternalUrlRejection::InvalidPercentEncoding);
            };
            let decoded = hi.as_u8().saturating_mul(16).saturating_add(lo.as_u8());
            if decoded < 0x20
                || decoded == 0x7F
                || decoded == b'\\'
                || hi_byte.is_ascii_lowercase()
                || lo_byte.is_ascii_lowercase()
            {
                return Err(ExternalUrlRejection::InvalidPercentEncoding);
            }
        }
        Ok(Self(value.to_owned()))
    }
}

#[derive(Debug)]
struct DecodedPercentPass(String);

impl DecodedPercentPass {
    fn new(input: &str) -> Self {
        let mut out = Vec::with_capacity(input.len());
        let mut iter = input.bytes();
        while let Some(byte) = iter.next() {
            if byte != b'%' {
                out.push(byte);
                continue;
            }
            let mut lookahead = iter.clone();
            if let (Some(hi_byte), Some(lo_byte)) = (lookahead.next(), lookahead.next())
                && let (Some(hi), Some(lo)) =
                    (HexNibble::try_new(hi_byte), HexNibble::try_new(lo_byte))
            {
                out.push(hi.as_u8().saturating_mul(16).saturating_add(lo.as_u8()));
                iter = lookahead;
                continue;
            }
            out.push(byte);
        }
        Self(String::from_utf8_lossy(&out).into_owned())
    }
}

#[derive(Debug)]
struct RedirectSafeParamValue(String);

impl RedirectSafeParamValue {
    fn try_new(value: &str) -> Result<Self, ExternalUrlRejection> {
        let forbidden_schemes = [
            "http:",
            "https:",
            "javascript:",
            "data:",
            "file:",
            "vbscript:",
            "blob:",
            "about:",
        ];
        let once = DecodedPercentPass::new(value);
        let twice = DecodedPercentPass::new(&once.0);
        for candidate in [value, once.0.as_str(), twice.0.as_str()] {
            let lowered = candidate.trim_start().to_ascii_lowercase();
            if lowered.contains("://")
                || lowered.starts_with("//")
                || lowered.contains('\\')
                || forbidden_schemes
                    .iter()
                    .any(|prefix| lowered.starts_with(prefix))
            {
                return Err(ExternalUrlRejection::OpenRedirectNotAllowed);
            }
        }
        Ok(Self(value.to_owned()))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalUrlQuery(String);

impl ExternalUrlQuery {
    pub fn try_new(value: &str) -> Result<Self, ExternalUrlRejection> {
        if !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@/?%".contains(&byte))
        {
            return Err(ExternalUrlRejection::ForbiddenCharacter);
        }
        if value.is_empty() || value.split('&').any(str::is_empty) {
            return Err(ExternalUrlRejection::InvalidQuery);
        }
        let validated = QueryPercentValidated::try_new(value)?;
        let mut rebuilt = Vec::new();
        for item in validated.0.split('&') {
            if let Some((key, tail)) = item.split_once('=') {
                let safe = RedirectSafeParamValue::try_new(tail)?;
                rebuilt.push(format!("{key}={}", safe.as_str()));
            } else {
                let safe = RedirectSafeParamValue::try_new(item)?;
                rebuilt.push(safe.as_str().to_owned());
            }
        }
        Ok(Self(rebuilt.join("&")))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalHttpsUrl(String);

impl CanonicalHttpsUrl {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedExternalUrl {
    scheme: ExternalUrlScheme,
    host: ExternalUrlHost,
    port: ExternalUrlPort,
    path: ExternalUrlPath,
    query: Option<ExternalUrlQuery>,
}

impl ParsedExternalUrl {
    pub fn parse(input: &ExternalUrlInput) -> Result<Self, ExternalUrlRejection> {
        let raw = input.as_str();
        if raw.is_empty() {
            return Err(ExternalUrlRejection::Empty);
        }
        if raw.len() > 8192 {
            return Err(ExternalUrlRejection::TooLong);
        }
        if !raw
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b":/?#[]@!$&'()*+,;=%-._~".contains(&byte))
        {
            return Err(ExternalUrlRejection::ForbiddenCharacter);
        }
        if raw.contains('#') {
            return Err(ExternalUrlRejection::FragmentNotAllowed);
        }
        let Some(after_scheme) = raw.strip_prefix("https://") else {
            return Err(ExternalUrlRejection::NonHttpsScheme);
        };
        let (before_query, query_opt) = match after_scheme.split_once('?') {
            Some((head, tail)) => (head, Some(tail)),
            None => (after_scheme, None),
        };
        let Some((authority, after_slash)) = before_query.split_once('/') else {
            let _host = ExternalUrlHost::try_new(before_query)?;
            return Err(ExternalUrlRejection::InvalidPath);
        };
        let host = ExternalUrlHost::try_new(authority)?;
        let path = ExternalUrlPath::try_new(&format!("/{after_slash}"))?;
        let query = match query_opt {
            Some(tail) => Some(ExternalUrlQuery::try_new(tail)?),
            None => None,
        };
        Ok(Self {
            scheme: ExternalUrlScheme::Https,
            host,
            port: ExternalUrlPort::ImplicitHttps,
            path,
            query,
        })
    }

    pub fn from_collected(url: &NormalizedUrl) -> Result<Self, ExternalUrlRejection> {
        Self::parse(&ExternalUrlInput::new(url.as_str()))
    }

    #[must_use]
    pub const fn scheme(&self) -> ExternalUrlScheme {
        self.scheme
    }

    #[must_use]
    pub const fn host(&self) -> &ExternalUrlHost {
        &self.host
    }

    #[must_use]
    pub const fn port(&self) -> ExternalUrlPort {
        self.port
    }

    #[must_use]
    pub const fn path(&self) -> &ExternalUrlPath {
        &self.path
    }

    #[must_use]
    pub const fn query(&self) -> Option<&ExternalUrlQuery> {
        self.query.as_ref()
    }

    #[must_use]
    pub fn canonical_url(&self) -> CanonicalHttpsUrl {
        let prefix = match self.scheme {
            ExternalUrlScheme::Https => "https://",
        };
        let mut rendered = format!("{prefix}{}{}", self.host.as_str(), self.path.as_str());
        if let Some(query) = &self.query {
            rendered.push('?');
            rendered.push_str(query.as_str());
        }
        CanonicalHttpsUrl(rendered)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedExternalUrl {
    AnthropicDataPrivacyControls,
}

impl FixedExternalUrl {
    #[must_use]
    pub fn parsed_url(self) -> ParsedExternalUrl {
        match self {
            Self::AnthropicDataPrivacyControls => ParsedExternalUrl {
                scheme: ExternalUrlScheme::Https,
                host: ExternalUrlHost(String::from("claude.ai")),
                port: ExternalUrlPort::ImplicitHttps,
                path: ExternalUrlPath(String::from("/settings/data-privacy-controls")),
                query: None,
            },
        }
    }

    #[must_use]
    pub fn match_parsed(candidate: &ParsedExternalUrl) -> Option<Self> {
        std::iter::once(Self::AnthropicDataPrivacyControls)
            .find(|fixed| candidate == &fixed.parsed_url())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalUrlAllowance {
    Fixed(FixedExternalUrl),
    Collected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedExternalUrl {
    url: ParsedExternalUrl,
    allowance: ExternalUrlAllowance,
}

impl AuthorizedExternalUrl {
    #[must_use]
    pub fn authorize_fixed(candidate: ParsedExternalUrl) -> Option<Self> {
        FixedExternalUrl::match_parsed(&candidate).map(|fixed| Self {
            url: candidate,
            allowance: ExternalUrlAllowance::Fixed(fixed),
        })
    }

    pub fn authorize_collected(
        requested: ParsedExternalUrl,
        collected: &NormalizedUrl,
    ) -> Result<Self, ExternalUrlRejection> {
        let verified = ParsedExternalUrl::from_collected(collected)?;
        if requested != verified {
            return Err(ExternalUrlRejection::NotAllowed);
        }
        Ok(Self {
            url: requested,
            allowance: ExternalUrlAllowance::Collected,
        })
    }

    #[must_use]
    pub const fn parsed(&self) -> &ParsedExternalUrl {
        &self.url
    }

    #[must_use]
    pub const fn allowance(&self) -> ExternalUrlAllowance {
        self.allowance
    }

    #[must_use]
    pub fn canonical_url(&self) -> CanonicalHttpsUrl {
        self.url.canonical_url()
    }
}
