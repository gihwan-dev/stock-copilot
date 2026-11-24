use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use application::{AiRunId, Clock, RunKind, SecretValue};
use domain::{Nanosecond, SpanSeconds, TimeSpan, Timestamp};
use subtle::ConstantTimeEq;
use tokio::sync::watch;
use zeroize::Zeroizing;

pub const TOKEN_RANDOM_BYTES: usize = 32_usize;
pub const TOKEN_HEX_BYTES: usize = 64_usize;
pub const AUTH_LINE_PREFIX: &[u8] = b"AUTH ";
pub const MIN_AUTH_LINE_BYTES: usize = 70_usize;
pub const DEFAULT_MAX_AUTH_LINE_BYTES: usize = 256_usize;
pub const MAX_AUTH_LINE_BYTES_CAP: usize = 1024_usize;
pub const DEFAULT_TOKEN_TTL_SECONDS: u64 = 1800_u64;
pub const MAX_TOKEN_TTL_SECONDS: u64 = 86_400_u64;

const TOKEN_STATE_ACTIVE: u8 = 0_u8;
const TOKEN_STATE_REVOKED: u8 = 1_u8;
const HEX_DIGITS: [u8; 16] = *b"0123456789abcdef";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenError {
    EntropyUnavailable,
    InvalidTtl,
    InvalidAuthLineLimit,
    ExpirationOverflow,
    CollectionRunNotAllowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TokenStatus {
    Active,
    Revoked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthRejection {
    TokenRevoked,
    TokenExpired,
    RunMismatch,
    AuthMissing,
    InvalidAuthFormat,
    MissingToken,
    InvalidToken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthLineLimit(usize);

impl AuthLineLimit {
    pub fn try_new(bytes: usize) -> Result<Self, TokenError> {
        if !(MIN_AUTH_LINE_BYTES..=MAX_AUTH_LINE_BYTES_CAP).contains(&bytes) {
            return Err(TokenError::InvalidAuthLineLimit);
        }
        Ok(Self(bytes))
    }

    #[must_use]
    pub const fn default_limit() -> Self {
        Self(DEFAULT_MAX_AUTH_LINE_BYTES)
    }

    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenTtl(TimeSpan);

impl TokenTtl {
    pub fn try_from_seconds(seconds: u64) -> Result<Self, TokenError> {
        if !(1_u64..=MAX_TOKEN_TTL_SECONDS).contains(&seconds) {
            return Err(TokenError::InvalidTtl);
        }
        TimeSpan::try_new(SpanSeconds::new(seconds), Nanosecond::new(0_u32))
            .map(Self)
            .map_err(|_error| TokenError::InvalidTtl)
    }

    pub fn default_ttl() -> Result<Self, TokenError> {
        Self::try_from_seconds(DEFAULT_TOKEN_TTL_SECONDS)
    }

    #[must_use]
    pub const fn as_time_span(self) -> TimeSpan {
        self.0
    }
}

pub struct RunAuthToken(SecretValue);

impl fmt::Debug for RunAuthToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RunAuthToken(<redacted>)")
    }
}

impl RunAuthToken {
    pub(crate) fn generate() -> Result<Self, TokenError> {
        let mut raw = Zeroizing::new([0_u8; TOKEN_RANDOM_BYTES]);
        getrandom::fill(raw.as_mut()).map_err(|_error| TokenError::EntropyUnavailable)?;
        let encoded = encode_hex(raw.as_ref())?;
        Ok(Self(SecretValue::new(encoded)))
    }

    #[must_use]
    pub fn as_exposed_bytes(&self) -> &[u8] {
        self.0.as_exposed_secret()
    }

    #[must_use]
    pub(crate) fn matches_constant_time(&self, candidate: &[u8]) -> TokenMatch {
        let expected = self.0.as_exposed_secret();
        let expected_len_ok = expected.len().ct_eq(&TOKEN_HEX_BYTES);
        let candidate_len_ok = candidate.len().ct_eq(&TOKEN_HEX_BYTES);
        let content_ok = expected.ct_eq(candidate);
        if bool::from(expected_len_ok & candidate_len_ok & content_ok) {
            TokenMatch::Matched
        } else {
            TokenMatch::Rejected
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TokenMatch {
    Matched,
    Rejected,
}

fn encode_hex(bytes: &[u8]) -> Result<Vec<u8>, TokenError> {
    let mut output = Vec::with_capacity(TOKEN_HEX_BYTES);
    for &byte in bytes {
        let high = usize::from(byte.wrapping_shr(4_u32));
        let low = usize::from(byte & 0x0f_u8);
        let Some(&high_char) = HEX_DIGITS.get(high) else {
            return Err(TokenError::EntropyUnavailable);
        };
        let Some(&low_char) = HEX_DIGITS.get(low) else {
            return Err(TokenError::EntropyUnavailable);
        };
        output.push(high_char);
        output.push(low_char);
    }
    Ok(output)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuthenticatedRun {
    run_id: AiRunId,
    run_kind: RunKind,
}

impl AuthenticatedRun {
    #[must_use]
    pub(crate) const fn run_id(&self) -> &AiRunId {
        &self.run_id
    }

    #[must_use]
    pub(crate) const fn run_kind(&self) -> RunKind {
        self.run_kind
    }
}

pub(crate) struct RunAuthBinding<C: Clock> {
    run_id: AiRunId,
    run_kind: RunKind,
    token: RunAuthToken,
    issued_at: Timestamp,
    expires_at: Timestamp,
    state: AtomicU8,
    revoke_tx: watch::Sender<TokenStatus>,
    clock: Arc<C>,
}

impl<C: Clock> fmt::Debug for RunAuthBinding<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RunAuthBinding")
            .field("run_id", &self.run_id)
            .field("run_kind", &self.run_kind)
            .field("token", &self.token)
            .field("issued_at", &self.issued_at)
            .field("expires_at", &self.expires_at)
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}

impl<C: Clock> RunAuthBinding<C> {
    pub(crate) fn issue(
        run_id: AiRunId,
        run_kind: RunKind,
        clock: Arc<C>,
        ttl: TokenTtl,
    ) -> Result<Self, TokenError> {
        match run_kind {
            RunKind::Analysis => {}
            RunKind::Collection => return Err(TokenError::CollectionRunNotAllowed),
        }
        let issued_at = clock.now();
        let expires_at = issued_at
            .checked_add(ttl.as_time_span())
            .map_err(|_error| TokenError::ExpirationOverflow)?;
        let token = RunAuthToken::generate()?;
        let (revoke_tx, _revoke_rx) = watch::channel(TokenStatus::Active);
        Ok(Self {
            run_id,
            run_kind,
            token,
            issued_at,
            expires_at,
            state: AtomicU8::new(TOKEN_STATE_ACTIVE),
            revoke_tx,
            clock,
        })
    }

    #[must_use]
    pub(crate) const fn run_id(&self) -> &AiRunId {
        &self.run_id
    }

    #[must_use]
    pub(crate) const fn run_kind(&self) -> RunKind {
        self.run_kind
    }

    #[must_use]
    pub(crate) const fn expires_at(&self) -> Timestamp {
        self.expires_at
    }

    #[must_use]
    pub(crate) const fn token(&self) -> &RunAuthToken {
        &self.token
    }

    #[must_use]
    pub(crate) const fn clock(&self) -> &Arc<C> {
        &self.clock
    }

    #[must_use]
    pub(crate) fn redact_token(&self, text: &str) -> String {
        let Ok(raw_token) = std::str::from_utf8(self.token.as_exposed_bytes()) else {
            return text.to_owned();
        };
        if text.contains(raw_token) {
            text.replace(raw_token, "[redacted]")
        } else {
            text.to_owned()
        }
    }

    #[must_use]
    pub(crate) fn status(&self) -> TokenStatus {
        match self.state.load(Ordering::Acquire) {
            TOKEN_STATE_ACTIVE => TokenStatus::Active,
            _other => TokenStatus::Revoked,
        }
    }

    #[must_use]
    pub(crate) fn subscribe_revocation(&self) -> watch::Receiver<TokenStatus> {
        self.revoke_tx.subscribe()
    }

    pub(crate) fn revoke(&self) {
        self.state.store(TOKEN_STATE_REVOKED, Ordering::Release);
        match self.revoke_tx.send(TokenStatus::Revoked) {
            Ok(()) => {}
            Err(_no_receivers) => {}
        }
    }

    #[must_use]
    pub(crate) fn remaining_until_expiry(&self) -> Duration {
        match self.status() {
            TokenStatus::Active => {}
            TokenStatus::Revoked => return Duration::ZERO,
        }
        let now = self.clock.now();
        match now.elapsed_until(self.expires_at) {
            Ok(span) => {
                let (secs, nanos) = span.as_parts();
                Duration::new(*secs.as_u64(), *nanos.as_u32())
            }
            Err(_elapsed_or_overflow) => Duration::ZERO,
        }
    }

    pub(crate) fn ensure_active(&self) -> Result<(), AuthRejection> {
        match self.status() {
            TokenStatus::Active => {}
            TokenStatus::Revoked => return Err(AuthRejection::TokenRevoked),
        }
        if self.clock.now() >= self.expires_at {
            return Err(AuthRejection::TokenExpired);
        }
        Ok(())
    }

    pub(crate) fn verify_auth_line(
        &self,
        line_bytes: &[u8],
        expected_run_id: &AiRunId,
    ) -> Result<AuthenticatedRun, AuthRejection> {
        self.ensure_active()?;
        if &self.run_id != expected_run_id {
            return Err(AuthRejection::RunMismatch);
        }
        let Some(body) = line_bytes.strip_suffix(b"\n") else {
            return Err(AuthRejection::AuthMissing);
        };
        if body.is_empty() {
            return Err(AuthRejection::AuthMissing);
        }
        let Some(candidate) = body.strip_prefix(AUTH_LINE_PREFIX) else {
            return Err(AuthRejection::InvalidAuthFormat);
        };
        if candidate.is_empty() {
            return Err(AuthRejection::MissingToken);
        }
        match self.token.matches_constant_time(candidate) {
            TokenMatch::Matched => Ok(AuthenticatedRun {
                run_id: self.run_id.clone(),
                run_kind: self.run_kind,
            }),
            TokenMatch::Rejected => Err(AuthRejection::InvalidToken),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use application::{AiRunId, RunKind};
    use domain::{Nanosecond, OffsetSeconds, SpanSeconds, Timestamp, UnixSeconds};
    use testkit_ports::SteppedClock;

    use super::{
        AuthLineLimit, AuthRejection, DEFAULT_MAX_AUTH_LINE_BYTES, MAX_AUTH_LINE_BYTES_CAP,
        MAX_TOKEN_TTL_SECONDS, MIN_AUTH_LINE_BYTES, RunAuthBinding, RunAuthToken, TokenError,
        TokenMatch, TokenStatus, TokenTtl,
    };

    fn timestamp_at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0_u32),
            OffsetSeconds::new(0_i32),
        )
        .expect("유효한 시각")
    }

    #[test]
    fn token_is_256_bit_hex_unique_and_redacted_in_debug() {
        let first = RunAuthToken::generate().expect("첫 토큰");
        let second = RunAuthToken::generate().expect("둘째 토큰");
        assert_eq!(
            first.as_exposed_bytes().len(),
            64_usize,
            "토큰 길이는 256비트 64바이트 16진수여야 한다"
        );
        assert!(
            first
                .as_exposed_bytes()
                .iter()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
            "소문자 16진수 문자만 사용한다"
        );
        assert_ne!(
            first.as_exposed_bytes(),
            second.as_exposed_bytes(),
            "두 번 생성한 토큰은 달라야 한다"
        );
        let debug_text = format!("{first:?}");
        let raw_text = std::str::from_utf8(first.as_exposed_bytes()).expect("UTF-8 토큰");
        assert_eq!(
            debug_text, "RunAuthToken(<redacted>)",
            "Debug 출력은 항상 가려야 한다"
        );
        assert!(
            !debug_text.contains(raw_text),
            "Debug 출력에 원문 토큰이 섞이면 안 된다"
        );
    }

    fn flip_hex_byte(byte: u8) -> u8 {
        if byte == b'0' { b'1' } else { b'0' }
    }

    #[test]
    fn constant_time_compare_rejects_bit_flips_wrong_case_and_wrong_length() {
        let token = RunAuthToken::generate().expect("토큰");
        let valid = token.as_exposed_bytes();
        assert_eq!(
            token.matches_constant_time(valid),
            TokenMatch::Matched,
            "정확히 일치하면 통과"
        );
        for index in [0_usize, 31_usize, 63_usize] {
            let mut mutated = valid.to_vec();
            if let Some(slot) = mutated.get_mut(index) {
                *slot = flip_hex_byte(*slot);
            }
            assert_eq!(
                token.matches_constant_time(&mutated),
                TokenMatch::Rejected,
                "한 바이트 변조 거부: index={index}"
            );
        }
        assert_eq!(
            token.matches_constant_time(&[]),
            TokenMatch::Rejected,
            "빈 슬라이스 거부"
        );
        if let Some(prefix) = valid.get(..63_usize) {
            assert_eq!(
                token.matches_constant_time(prefix),
                TokenMatch::Rejected,
                "짧은 접두사 거부"
            );
        }
        let mut longer = valid.to_vec();
        longer.push(b'0');
        assert_eq!(
            token.matches_constant_time(&longer),
            TokenMatch::Rejected,
            "긴 후보 거부"
        );
    }

    #[test]
    fn ttl_and_auth_line_limit_validate_bounds() {
        assert_eq!(
            TokenTtl::try_from_seconds(0_u64),
            Err(TokenError::InvalidTtl),
            "0초 TTL 거부"
        );
        assert_eq!(
            TokenTtl::try_from_seconds(MAX_TOKEN_TTL_SECONDS.saturating_add(1_u64)),
            Err(TokenError::InvalidTtl),
            "상한 초과 TTL 거부"
        );
        assert_eq!(
            TokenTtl::try_from_seconds(1_u64).map(|ttl| ttl.as_time_span().as_parts().0),
            Ok(SpanSeconds::new(1_u64)),
            "1초 TTL 허용"
        );
        assert_eq!(
            TokenTtl::try_from_seconds(MAX_TOKEN_TTL_SECONDS)
                .map(|ttl| ttl.as_time_span().as_parts().0),
            Ok(SpanSeconds::new(MAX_TOKEN_TTL_SECONDS)),
            "상한 경계 TTL 허용"
        );
        assert_eq!(
            AuthLineLimit::try_new(MIN_AUTH_LINE_BYTES.saturating_sub(1_usize)),
            Err(TokenError::InvalidAuthLineLimit),
            "최소 길이 미만 거부"
        );
        assert_eq!(
            AuthLineLimit::try_new(MAX_AUTH_LINE_BYTES_CAP.saturating_add(1_usize)),
            Err(TokenError::InvalidAuthLineLimit),
            "최대 길이 초과 거부"
        );
        assert_eq!(
            AuthLineLimit::default_limit().as_usize(),
            DEFAULT_MAX_AUTH_LINE_BYTES,
            "기본 인증 줄 상한"
        );
    }

    #[test]
    fn run_auth_binding_rejects_collection_and_verifies_run_id() {
        let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
        let run_id = AiRunId::try_new("run-analysis-1").expect("실행 ID");
        let other_run_id = AiRunId::try_new("run-analysis-2").expect("다른 실행 ID");
        let ttl = TokenTtl::try_from_seconds(60_u64).expect("유효 TTL");

        let collection =
            RunAuthBinding::issue(run_id.clone(), RunKind::Collection, Arc::clone(&clock), ttl);
        assert_eq!(
            collection.err(),
            Some(TokenError::CollectionRunNotAllowed),
            "수집 실행에는 MCP 토큰을 발급하지 않는다"
        );

        let binding =
            RunAuthBinding::issue(run_id.clone(), RunKind::Analysis, clock, ttl).expect("발급");
        assert_eq!(
            binding.remaining_until_expiry(),
            Duration::from_secs(60_u64),
            "잔여 유효 시간 계산"
        );
        let binding_debug = format!("{binding:?}");
        let raw = std::str::from_utf8(binding.token().as_exposed_bytes()).expect("UTF-8");
        assert!(
            binding_debug.contains("RunAuthBinding")
                && binding_debug.contains("RunAuthToken(<redacted>)")
                && !binding_debug.contains(raw),
            "RunAuthBinding Debug는 구조와 가림 표식을 출력하고 원문을 노출하지 않는다"
        );

        let mut valid_line = b"AUTH ".to_vec();
        valid_line.extend_from_slice(binding.token().as_exposed_bytes());
        valid_line.push(b'\n');

        assert_eq!(
            binding.verify_auth_line(&valid_line, &other_run_id),
            Err(AuthRejection::RunMismatch),
            "다른 실행 ID 거부"
        );
        let ok = binding
            .verify_auth_line(&valid_line, &run_id)
            .expect("일치하는 실행 인증 성공");
        assert_eq!(ok.run_id(), &run_id, "인증된 실행 ID 일치");
        assert_eq!(ok.run_kind(), RunKind::Analysis, "분석 실행 확인");
    }

    #[test]
    fn run_auth_binding_rejects_malformed_auth_lines() {
        let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
        let run_id = AiRunId::try_new("run-analysis-fmt").expect("실행 ID");
        let ttl = TokenTtl::try_from_seconds(60_u64).expect("유효 TTL");
        let binding =
            RunAuthBinding::issue(run_id.clone(), RunKind::Analysis, clock, ttl).expect("발급");
        let mut crlf = b"AUTH ".to_vec();
        crlf.extend_from_slice(binding.token().as_exposed_bytes());
        crlf.extend_from_slice(b"\r\n");

        let cases: [(&[u8], AuthRejection, &str); 6] = [
            (b"", AuthRejection::AuthMissing, "빈 입력 거부"),
            (b"\n", AuthRejection::AuthMissing, "빈 줄 거부"),
            (
                b"BEARER abc\n",
                AuthRejection::InvalidAuthFormat,
                "접두사 오류 거부",
            ),
            (b"AUTH \n", AuthRejection::MissingToken, "토큰 누락 거부"),
            (
                b"AUTH wrong\n",
                AuthRejection::InvalidToken,
                "잘못된 토큰 거부",
            ),
            (&crlf, AuthRejection::InvalidToken, "CRLF 토큰 거부"),
        ];
        for (line, expected, msg) in cases {
            assert_eq!(
                binding.verify_auth_line(line, &run_id),
                Err(expected),
                "{msg}"
            );
        }
    }

    #[test]
    fn run_auth_binding_enforces_expiry_and_revocation_notification() {
        let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
        let run_id = AiRunId::try_new("run-ttl-1").expect("실행 ID");
        let ttl = TokenTtl::try_from_seconds(30_u64).expect("유효 TTL");
        let binding =
            RunAuthBinding::issue(run_id.clone(), RunKind::Analysis, Arc::clone(&clock), ttl)
                .expect("발급");
        let mut line = b"AUTH ".to_vec();
        line.extend_from_slice(binding.token().as_exposed_bytes());
        line.push(b'\n');

        clock
            .advance_to(timestamp_at(1_700_000_029_i64))
            .expect("시각 이동");
        assert_eq!(
            binding.remaining_until_expiry(),
            Duration::from_secs(1_u64),
            "만료 1초 전 잔여 시간"
        );
        assert!(
            binding.verify_auth_line(&line, &run_id).is_ok(),
            "만료 1초 전 허용"
        );

        clock
            .advance_to(timestamp_at(1_700_000_030_i64))
            .expect("시각 이동");
        assert_eq!(
            binding.remaining_until_expiry(),
            Duration::ZERO,
            "만료 시각 잔여 0"
        );
        assert_eq!(
            binding.verify_auth_line(&line, &run_id),
            Err(AuthRejection::TokenExpired),
            "만료 경계 시각 즉시 거부"
        );

        let clock_revoke = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
        let binding_revoke =
            RunAuthBinding::issue(run_id.clone(), RunKind::Analysis, clock_revoke, ttl)
                .expect("폐기 테스트 발급");
        let mut revoke_line = b"AUTH ".to_vec();
        revoke_line.extend_from_slice(binding_revoke.token().as_exposed_bytes());
        revoke_line.push(b'\n');
        let rx = binding_revoke.subscribe_revocation();
        assert_eq!(*rx.borrow(), TokenStatus::Active, "초기 상태 활성");
        binding_revoke.revoke();
        assert_eq!(*rx.borrow(), TokenStatus::Revoked, "폐기 알림 전달");
        assert_eq!(
            binding_revoke.remaining_until_expiry(),
            Duration::ZERO,
            "폐기 후 잔여 0"
        );
        assert_eq!(
            binding_revoke.verify_auth_line(&revoke_line, &run_id),
            Err(AuthRejection::TokenRevoked),
            "폐기된 토큰 거부"
        );
    }
}
