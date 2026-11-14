use crate::{
    Currency, Market, Nanosecond, OrderIntent, PriceObservation, ProposalId, ProposalLifetime,
    ProposalPriceRange, TimeError, TimeSpan, Timestamp,
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Draft;
#[derive(Debug)]
pub struct Validated {
    validated_at: Timestamp,
    observation: PriceObservation,
}
pub struct Proposal<State> {
    intent: OrderIntent,
    id: ProposalId,
    created_at: Timestamp,
    expires_at: Timestamp,
    range: ProposalPriceRange,
    state: State,
}
impl<State> fmt::Debug for Proposal<State> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Proposal")
            .field("id", &self.id)
            .field("created_at", &self.created_at)
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckResult {
    Passed,
    Failed,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationCheck {
    Instrument,
    MarketHours,
    AvailableFundsOrQuantity,
    OppositePendingOrder,
    BuyWarnings,
    Principles,
    OrderPrice,
    MarketOrderPermission,
    AccountMatch,
    PriceObservation,
    Expiry,
    PriceRange,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationReport {
    instrument: CheckResult,
    market_hours: CheckResult,
    available_funds_or_quantity: CheckResult,
    opposite_pending_order: CheckResult,
    buy_warnings: CheckResult,
    principles: CheckResult,
    order_price: CheckResult,
    market_order_permission: CheckResult,
    account_match: CheckResult,
    price_observation: CheckResult,
    expiry: CheckResult,
    price_range: CheckResult,
}
impl ValidationReport {
    #[must_use]
    pub fn result(&self, check: ValidationCheck) -> CheckResult {
        match check {
            ValidationCheck::Instrument => self.instrument,
            ValidationCheck::MarketHours => self.market_hours,
            ValidationCheck::AvailableFundsOrQuantity => self.available_funds_or_quantity,
            ValidationCheck::OppositePendingOrder => self.opposite_pending_order,
            ValidationCheck::BuyWarnings => self.buy_warnings,
            ValidationCheck::Principles => self.principles,
            ValidationCheck::OrderPrice => self.order_price,
            ValidationCheck::MarketOrderPermission => self.market_order_permission,
            ValidationCheck::AccountMatch => self.account_match,
            ValidationCheck::PriceObservation => self.price_observation,
            ValidationCheck::Expiry => self.expiry,
            ValidationCheck::PriceRange => self.price_range,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalError {
    Time(TimeError),
    CurrencyMismatch,
    OrderKeyMismatch,
    InstrumentMismatch,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalExecutionError {
    ClockRollback,
    Expired,
    PriceOutsideRange,
    ObservationInvalid,
}
impl<State> Proposal<State> {
    #[must_use]
    pub fn intent(&self) -> &OrderIntent {
        &self.intent
    }
    #[must_use]
    pub fn id(&self) -> &ProposalId {
        &self.id
    }
    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.created_at
    }
    #[must_use]
    pub fn expires_at(&self) -> Timestamp {
        self.expires_at
    }
    fn expiry(&self, now: Timestamp) -> CheckResult {
        if now < self.created_at || now >= self.expires_at {
            CheckResult::Failed
        } else {
            CheckResult::Passed
        }
    }
}
impl Proposal<Draft> {
    pub fn new(
        id: ProposalId,
        intent: OrderIntent,
        created_at: Timestamp,
        range: ProposalPriceRange,
        lifetime: ProposalLifetime,
    ) -> Result<Self, ProposalError> {
        if id.as_client_order_id() != intent.client_order_id() {
            return Err(ProposalError::OrderKeyMismatch);
        }
        let currency = if intent.market() == Market::Kr {
            Currency::Krw
        } else {
            Currency::Usd
        };
        if range.instrument() != intent.instrument() {
            return Err(ProposalError::InstrumentMismatch);
        }
        if range.currency() != currency {
            return Err(ProposalError::CurrencyMismatch);
        }
        let expires_at = TimeSpan::try_new(lifetime.seconds(), Nanosecond::new(0))
            .and_then(|span| created_at.checked_add(span))
            .map_err(ProposalError::Time)?;
        Ok(Self {
            intent,
            id,
            created_at,
            expires_at,
            range,
            state: Draft,
        })
    }
    #[must_use]
    pub fn report(&self, now: Timestamp, current: &PriceObservation) -> ValidationReport {
        ValidationReport {
            instrument: CheckResult::Unknown,
            market_hours: CheckResult::Unknown,
            available_funds_or_quantity: CheckResult::Unknown,
            opposite_pending_order: CheckResult::Unknown,
            buy_warnings: CheckResult::Unknown,
            principles: CheckResult::Unknown,
            order_price: CheckResult::Unknown,
            market_order_permission: CheckResult::Unknown,
            account_match: CheckResult::Unknown,
            price_observation: current.check(self.intent.instrument(), now),
            expiry: self.expiry(now),
            price_range: self.range.check(current.price()),
        }
    }
    pub fn validate(
        self,
        now: Timestamp,
        current: PriceObservation,
    ) -> Result<Proposal<Validated>, ValidationReport> {
        let report = self.report(now, &current);
        self.finish_validation(report, now, current)
    }
    fn finish_validation(
        self,
        report: ValidationReport,
        now: Timestamp,
        observation: PriceObservation,
    ) -> Result<Proposal<Validated>, ValidationReport> {
        let ValidationReport {
            instrument,
            market_hours,
            available_funds_or_quantity,
            opposite_pending_order,
            buy_warnings,
            principles,
            order_price,
            market_order_permission,
            account_match,
            price_observation,
            expiry,
            price_range,
        } = report;
        if [
            instrument,
            market_hours,
            available_funds_or_quantity,
            opposite_pending_order,
            buy_warnings,
            principles,
            order_price,
            market_order_permission,
            account_match,
            price_observation,
            expiry,
            price_range,
        ]
        .into_iter()
        .any(|result| result != CheckResult::Passed)
        {
            return Err(report);
        }
        Ok(Proposal {
            intent: self.intent,
            id: self.id,
            created_at: self.created_at,
            expires_at: self.expires_at,
            range: self.range,
            state: Validated {
                validated_at: now,
                observation,
            },
        })
    }
}
impl Proposal<Validated> {
    pub fn into_execution(
        self,
        now: Timestamp,
        current: PriceObservation,
    ) -> Result<ExecutionToken, ProposalExecutionError> {
        if now < self.state.validated_at {
            return Err(ProposalExecutionError::ClockRollback);
        }
        if self.expiry(now) != CheckResult::Passed {
            return Err(ProposalExecutionError::Expired);
        }
        if current.check(self.intent.instrument(), now) != CheckResult::Passed {
            return Err(ProposalExecutionError::ObservationInvalid);
        }
        if self.range.check(current.price()) != CheckResult::Passed {
            return Err(ProposalExecutionError::PriceOutsideRange);
        }
        Ok(ExecutionToken {
            proposal: self,
            checked_at: now,
            execution_observation: current,
        })
    }
}

#[derive(Debug)]
pub struct ExecutionToken {
    proposal: Proposal<Validated>,
    checked_at: Timestamp,
    execution_observation: PriceObservation,
}
impl ExecutionToken {
    pub fn recheck(
        self,
        now: Timestamp,
        current: PriceObservation,
    ) -> Result<Self, ProposalExecutionError> {
        if now < self.checked_at {
            return Err(ProposalExecutionError::ClockRollback);
        }
        self.proposal.into_execution(now, current)
    }
    #[must_use]
    pub fn expires_at(&self) -> Timestamp {
        self.proposal.expires_at()
    }

    #[must_use]
    pub fn intent(&self) -> &OrderIntent {
        self.proposal.intent()
    }
    #[must_use]
    pub fn id(&self) -> &ProposalId {
        self.proposal.id()
    }
    #[must_use]
    pub fn validated_at(&self) -> Timestamp {
        self.proposal.state.validated_at
    }
    #[must_use]
    pub fn checked_at(&self) -> Timestamp {
        self.checked_at
    }
    #[must_use]
    pub fn validation_observation(&self) -> &PriceObservation {
        &self.proposal.state.observation
    }
    #[must_use]
    pub fn execution_observation(&self) -> &PriceObservation {
        &self.execution_observation
    }
}

#[cfg(test)]
pub(crate) mod summary_tests {
    use super::{
        CheckResult, ExecutionToken, Nanosecond, OrderIntent, PriceObservation, Proposal,
        ProposalId, ProposalLifetime, ProposalPriceRange, Timestamp,
    };
    pub(crate) fn token(intent: OrderIntent, price: crate::Price) -> ExecutionToken {
        let at = Timestamp::from_parts(
            crate::UnixSeconds::new(1),
            Nanosecond::new(0),
            crate::OffsetSeconds::new(0),
        )
        .expect("시각");
        let observation =
            PriceObservation::from_quote_parts(intent.instrument().clone(), price, at)
                .expect("관측");
        let ratio =
            crate::Ratio::parse(&crate::NumericInput::try_new("1").expect("입력")).expect("비율");
        let facts = crate::EstimateRangeFacts {
            instrument: intent.instrument().clone(),
            reference: price,
            volatility: crate::ProposalVolatility::try_new(ratio).expect("변동성"),
            horizon: crate::TradingDayHorizon::try_new(ratio).expect("기간"),
        };
        let range =
            ProposalPriceRange::from_estimate_task(facts, observation.clone()).expect("범위");
        let id = ProposalId::new(intent.client_order_id().clone());
        let draft =
            Proposal::new(id, intent, at, range, ProposalLifetime::default()).expect("제안");
        let mut report = draft.report(at, &observation);
        report.instrument = CheckResult::Passed;
        report.market_hours = CheckResult::Passed;
        report.available_funds_or_quantity = CheckResult::Passed;
        report.opposite_pending_order = CheckResult::Passed;
        report.buy_warnings = CheckResult::Passed;
        report.principles = CheckResult::Passed;
        report.order_price = CheckResult::Passed;
        report.market_order_permission = CheckResult::Passed;
        report.account_match = CheckResult::Passed;
        draft
            .finish_validation(report, at, observation.clone())
            .expect("검증")
            .into_execution(at, observation)
            .expect("토큰")
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CheckResult, Draft, Proposal, ProposalError, ProposalExecutionError, ValidationCheck,
        ValidationReport,
    };
    use crate::{
        AccountId, ClientOrderId, Currency, EstimateRangeFacts, Market, Nanosecond, NumericInput,
        OffsetSeconds, OrderIntent, OrderTerms, Price, PriceObservation, ProposalId,
        ProposalLifetime, ProposalPriceRange, ProposalVolatility, Ratio, ResolvedOrderInstrument,
        SpanSeconds, Symbol, Timestamp, TradingDayHorizon, UnixSeconds, WholeOrderQuantity,
    };
    use proptest::test_runner::{Config, TestRunner};

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn price(text: &str) -> Price {
        Price::parse(&NumericInput::try_new(text).expect("입력"), Currency::Usd).expect("가격")
    }
    fn ratio(text: &str) -> Ratio {
        Ratio::parse(&NumericInput::try_new(text).expect("입력")).expect("비율")
    }
    fn instrument(symbol: &str) -> ResolvedOrderInstrument {
        ResolvedOrderInstrument::from_master_lookup_result(
            Symbol::try_new(symbol).expect("종목"),
            Market::Us,
        )
        .expect("시장")
    }
    fn observation(text: &str, seconds: i64) -> PriceObservation {
        PriceObservation::from_quote_parts(instrument("TEST"), price(text), at(seconds))
            .expect("관측")
    }
    fn estimate_facts(
        instrument: ResolvedOrderInstrument,
        reference: Price,
        volatility: ProposalVolatility,
        horizon: TradingDayHorizon,
    ) -> EstimateRangeFacts {
        EstimateRangeFacts {
            instrument,
            reference,
            volatility,
            horizon,
        }
    }
    fn range(base: &str, sigma: &str, horizon: &str) -> ProposalPriceRange {
        ProposalPriceRange::from_estimate_task(
            estimate_facts(
                instrument("TEST"),
                price(base),
                ProposalVolatility::try_new(ratio(sigma)).expect("변동성"),
                TradingDayHorizon::try_new(ratio(horizon)).expect("기간"),
            ),
            observation(base, 0),
        )
        .expect("범위")
    }
    fn whole_quantity() -> WholeOrderQuantity {
        WholeOrderQuantity::try_new(
            crate::Quantity::parse(&NumericInput::try_new("1").expect("입력")).expect("수량"),
        )
        .expect("주문 수량")
    }
    fn draft() -> Proposal<Draft> {
        let id = ProposalId::new(ClientOrderId::try_new("proposal-1").expect("ID"));
        let intent = OrderIntent::new(
            AccountId::new(987_654_321),
            instrument("TEST"),
            id.as_client_order_id().clone(),
            OrderTerms::UsMarketBuy {
                quantity: whole_quantity(),
            },
        )
        .expect("주문");
        Proposal::new(
            id,
            intent,
            at(0),
            range("100", "0.02", "4"),
            ProposalLifetime::default(),
        )
        .expect("제안")
    }
    fn passed(
        proposal: &Proposal<Draft>,
        now: Timestamp,
        current: &PriceObservation,
    ) -> ValidationReport {
        let mut report = proposal.report(now, current);
        report.instrument = CheckResult::Passed;
        report.market_hours = CheckResult::Passed;
        report.available_funds_or_quantity = CheckResult::Passed;
        report.opposite_pending_order = CheckResult::Passed;
        report.buy_warnings = CheckResult::Passed;
        report.principles = CheckResult::Passed;
        report.order_price = CheckResult::Passed;
        report.market_order_permission = CheckResult::Passed;
        report.account_match = CheckResult::Passed;
        report
    }
    #[test]
    fn m14_execution_token_preserves_identity_observations_and_rejects_rollback() {
        let proposal = draft();
        let quote = observation("100", 5);
        let report = passed(&proposal, at(5), &quote);
        let validated = proposal
            .finish_validation(report, at(5), quote)
            .expect("검증");
        assert_eq!(
            validated
                .into_execution(at(3), observation("100", 3))
                .expect_err("검증 이후 역행"),
            ProposalExecutionError::ClockRollback
        );
        let next = draft();
        let current = observation("100", 5);
        let cleared = passed(&next, at(5), &current);
        let token = next
            .finish_validation(cleared, at(5), current.clone())
            .expect("검증")
            .into_execution(at(6), observation("101", 6))
            .expect("토큰");
        assert_eq!(
            token.intent().client_order_id(),
            token.id().as_client_order_id()
        );
        assert_eq!(token.validated_at(), at(5));
        assert_eq!(token.checked_at(), at(6));
        assert_eq!(token.validation_observation(), &current);
        assert_eq!(token.execution_observation(), &observation("101", 6));
        let rendered = format!("{token:?}");
        assert!(rendered.contains("proposal-1"));
        assert!(!rendered.contains("987654321"));
    }
    fn execution_token() -> crate::ExecutionToken {
        let proposal = draft();
        let current = observation("100", 0);
        let report = passed(&proposal, at(0), &current);
        proposal
            .finish_validation(report, at(0), current)
            .expect("검증")
            .into_execution(at(2), observation("100", 2))
            .expect("소비")
    }
    #[test]
    fn token_recheck_consumes_without_extracting_order_and_enforces_deadline() {
        let token = execution_token();
        let original = token.intent().clone();
        assert_eq!(token.expires_at(), at(600));
        let checked = token.recheck(at(3), observation("101", 3)).expect("재확인");
        assert_eq!(checked.intent(), &original);
        assert_eq!(checked.checked_at(), at(3));
        assert_eq!(checked.validation_observation(), &observation("100", 0));
        assert_eq!(checked.execution_observation(), &observation("101", 3));
        assert_eq!(
            execution_token()
                .recheck(at(1), observation("100", 1))
                .expect_err("소비 이후 역행"),
            ProposalExecutionError::ClockRollback
        );
        assert_eq!(
            execution_token()
                .recheck(at(600), observation("100", 600))
                .expect_err("전송 전 만료"),
            ProposalExecutionError::Expired
        );
        let _maximum = ProposalLifetime::try_new(SpanSeconds::new(600)).expect("설정 상한");
    }
    #[test]
    fn expiry_settings_and_observation_boundaries_fail_closed() {
        let _minimum = ProposalLifetime::try_new(SpanSeconds::new(1)).expect("최소 기간");
        assert_eq!(
            ProposalLifetime::try_new(SpanSeconds::new(0)),
            Err(crate::ProposalLifetimeError::OutOfRange)
        );
        assert_eq!(
            ProposalLifetime::try_new(SpanSeconds::new(601)),
            Err(crate::ProposalLifetimeError::OutOfRange)
        );
        let proposal = draft();
        assert_eq!(proposal.created_at(), at(0));
        assert_eq!(proposal.expires_at(), at(600));
        for seconds in [-1, 600, 601] {
            assert_eq!(
                proposal
                    .report(at(seconds), &observation("100", seconds))
                    .result(ValidationCheck::Expiry),
                CheckResult::Failed
            );
        }
        let boundary = at(600)
            .checked_sub(crate::TimeSpan::NANOSECOND)
            .expect("경계");
        assert_eq!(
            proposal
                .report(boundary, &observation("100", 599))
                .result(ValidationCheck::Expiry),
            CheckResult::Passed
        );
        for quote in [
            observation("100", 1),
            observation("100", -5),
            PriceObservation::from_quote_parts(instrument("OTHER"), price("100"), at(0))
                .expect("다른 종목"),
        ] {
            assert_eq!(
                proposal
                    .report(at(0), &quote)
                    .result(ValidationCheck::PriceObservation),
                CheckResult::Failed
            );
        }
        assert_eq!(
            proposal
                .report(at(4), &observation("100", 0))
                .result(ValidationCheck::PriceObservation),
            CheckResult::Passed
        );
        let fresh = observation("100", 0);
        let cleared = passed(&proposal, at(0), &fresh);
        assert_eq!(
            proposal
                .finish_validation(cleared, at(0), fresh)
                .expect("검증")
                .into_execution(at(5), observation("100", 0))
                .expect_err("오래된 관측"),
            ProposalExecutionError::ObservationInvalid
        );
    }
    fn set_result(report: &mut ValidationReport, check: ValidationCheck, result: CheckResult) {
        let field = match check {
            ValidationCheck::Instrument => &mut report.instrument,
            ValidationCheck::MarketHours => &mut report.market_hours,
            ValidationCheck::AvailableFundsOrQuantity => &mut report.available_funds_or_quantity,
            ValidationCheck::OppositePendingOrder => &mut report.opposite_pending_order,
            ValidationCheck::BuyWarnings => &mut report.buy_warnings,
            ValidationCheck::Principles => &mut report.principles,
            ValidationCheck::OrderPrice => &mut report.order_price,
            ValidationCheck::MarketOrderPermission => &mut report.market_order_permission,
            ValidationCheck::AccountMatch => &mut report.account_match,
            ValidationCheck::Expiry => &mut report.expiry,
            ValidationCheck::PriceRange => &mut report.price_range,
            ValidationCheck::PriceObservation => &mut report.price_observation,
        };
        *field = result;
    }
    #[test]
    fn m21_each_unknown_or_failed_field_blocks_and_public_validation_cannot_pass() {
        let proposal = draft();
        let fresh = observation("100", 0);
        let pending = proposal.report(at(0), &fresh);
        assert_eq!(
            proposal.validate(at(0), fresh).expect_err("미확인"),
            pending
        );
        for check in [
            ValidationCheck::Instrument,
            ValidationCheck::MarketHours,
            ValidationCheck::AvailableFundsOrQuantity,
            ValidationCheck::OppositePendingOrder,
            ValidationCheck::BuyWarnings,
            ValidationCheck::Principles,
            ValidationCheck::OrderPrice,
            ValidationCheck::MarketOrderPermission,
            ValidationCheck::AccountMatch,
            ValidationCheck::Expiry,
            ValidationCheck::PriceRange,
            ValidationCheck::PriceObservation,
        ] {
            for result in [CheckResult::Unknown, CheckResult::Failed] {
                let next = draft();
                let current = observation("100", 0);
                let mut report = passed(&next, at(0), &current);
                set_result(&mut report, check, result);
                assert_eq!(report.result(check), result);
                assert_eq!(
                    next.finish_validation(report.clone(), at(0), current)
                        .expect_err("통과 불가"),
                    report
                );
            }
        }
    }
    fn assert_realistic_base(base: &str) {
        for sigma in [
            "0.005",
            "0.0213456789",
            "0.0123456789012345678901234567",
            "0.1",
            "0.3333333333333333333333333333",
            "0.00000000000001",
        ] {
            for horizon in ["1", "250", "0.125", "20.5"] {
                assert_eq!(
                    range(base, sigma, horizon).check(price(base)),
                    CheckResult::Passed
                );
            }
        }
    }
    #[test]
    fn realistic_precision_and_exact_price_boundaries_are_usable() {
        for base in ["71234.5", "187.23", "1000000", "0.01"] {
            assert_realistic_base(base);
        }
        for (text, expected) in [
            ("98", CheckResult::Passed),
            ("102", CheckResult::Passed),
            ("97.99999999", CheckResult::Failed),
            ("102.00000001", CheckResult::Failed),
        ] {
            assert_eq!(range("100", "0.02", "4").check(price(text)), expected);
        }
        assert_eq!(
            range("79228162514264337593543950335", "0.1", "250")
                .check(price("79228162514264337593543950335")),
            CheckResult::Passed
        );
    }

    fn digits(value: i128) -> Vec<u8> {
        value
            .unsigned_abs()
            .to_string()
            .bytes()
            .rev()
            .map(|byte| byte.checked_sub(b'0').expect("숫자"))
            .collect()
    }
    fn decimal_product(left: &[u8], right: &[u8]) -> Vec<u8> {
        let mut result = vec![0_u8; left.len().checked_add(right.len()).expect("크기")];
        for (index, first) in left.iter().enumerate() {
            let mut carry = 0_u16;
            for (offset, second) in right.iter().enumerate() {
                let target = result
                    .get_mut(index.checked_add(offset).expect("위치"))
                    .expect("자리");
                let total = u16::from(*first)
                    .checked_mul(u16::from(*second))
                    .and_then(|value| value.checked_add(u16::from(*target)))
                    .and_then(|value| value.checked_add(carry))
                    .expect("자리 곱");
                *target = u8::try_from(total.checked_rem(10).expect("나머지")).expect("한 자리");
                carry = total.checked_div(10).expect("올림");
            }
            *result
                .get_mut(index.checked_add(right.len()).expect("올림 위치"))
                .expect("올림 자리") = u8::try_from(carry).expect("올림");
        }
        while result.last() == Some(&0) {
            let _removed = result.pop();
        }
        result
    }
    fn independent_range(base: i128, current: i128, sigma: Ratio, horizon: Ratio) -> CheckResult {
        let difference = current.checked_sub(base).expect("차이");
        let mut left = digits(
            difference
                .checked_pow(2)
                .and_then(|value| value.checked_mul(4))
                .expect("차이 제곱"),
        );
        let places = sigma
            .as_decimal()
            .scale()
            .checked_mul(2)
            .and_then(|value| value.checked_add(horizon.as_decimal().scale()))
            .expect("스케일");
        let mut shifted = vec![0_u8; usize::try_from(places).expect("자리 수")];
        shifted.extend(left);
        left = shifted;
        let coefficient = digits(sigma.as_decimal().mantissa());
        let reference = digits(base);
        let right = decimal_product(
            &decimal_product(
                &decimal_product(
                    &decimal_product(&coefficient, &coefficient),
                    &digits(horizon.as_decimal().mantissa()),
                ),
                &reference,
            ),
            &reference,
        );
        while left.last() == Some(&0) {
            let _removed = left.pop();
        }
        if left
            .len()
            .cmp(&right.len())
            .then_with(|| left.iter().rev().cmp(right.iter().rev()))
            .is_le()
        {
            CheckResult::Passed
        } else {
            CheckResult::Failed
        }
    }
    #[test]
    fn long_decimal_volatility_matches_independent_rational_reference() {
        let mut runner = TestRunner::new(Config::with_cases(4096));
        runner
            .run(
                &(
                    1_i64..1_000_000,
                    1_i64..1_000_000,
                    50_000_u64..1_000_001,
                    0_u128..1_000_000_000_000_000_000_000,
                    1_u32..250,
                    0_u32..1000,
                ),
                |(base, current, lead, tail, days, fraction)| {
                    let sigma = ratio(&format!("0.{lead:07}{tail:021}"));
                    let horizon = ratio(&format!("{days}.{fraction:03}"));
                    let reference = format!("{base}.23");
                    let quote = format!("{current}.17");
                    let actual = range(
                        &reference,
                        &format!("0.{lead:07}{tail:021}"),
                        &format!("{days}.{fraction:03}"),
                    )
                    .check(price(&quote));
                    let base_coefficient = i128::from(base)
                        .checked_mul(100)
                        .and_then(|value| value.checked_add(23))
                        .expect("기준가");
                    let quote_coefficient = i128::from(current)
                        .checked_mul(100)
                        .and_then(|value| value.checked_add(17))
                        .expect("현재가");
                    proptest::prop_assert_eq!(
                        actual,
                        independent_range(base_coefficient, quote_coefficient, sigma, horizon)
                    );
                    Ok(())
                },
            )
            .expect("유리수 기준");
    }
    #[test]
    fn constructors_currency_identity_time_and_consumption_errors_are_closed() {
        for text in ["0", "-1"] {
            assert_eq!(
                ProposalVolatility::try_new(ratio(text)),
                Err(crate::ProposalRangeError::NonPositive)
            );
            assert_eq!(
                TradingDayHorizon::try_new(ratio(text)),
                Err(crate::ProposalRangeError::NonPositive)
            );
        }
        let proposal = draft();
        assert_eq!(
            Proposal::new(
                ProposalId::new(ClientOrderId::try_new("other").expect("ID")),
                proposal.intent().clone(),
                at(0),
                proposal.range.clone(),
                ProposalLifetime::default()
            )
            .expect_err("키"),
            ProposalError::OrderKeyMismatch
        );
        assert_eq!(
            Proposal::new(
                proposal.id().clone(),
                proposal.intent().clone(),
                at(253_402_300_799),
                proposal.range.clone(),
                ProposalLifetime::default()
            )
            .expect_err("넘침"),
            ProposalError::Time(crate::TimeError::Overflow)
        );
        assert_eq!(
            PriceObservation::from_quote_parts(
                instrument("TEST"),
                price("100"),
                at(253_402_300_799)
            )
            .expect_err("관측 시한 넘침"),
            crate::TimeError::Overflow
        );
        for (seconds, text, error) in [
            (600, "100", ProposalExecutionError::Expired),
            (1, "103", ProposalExecutionError::PriceOutsideRange),
        ] {
            let next = draft();
            let quote = observation("100", 0);
            let report = passed(&next, at(0), &quote);
            assert_eq!(
                next.finish_validation(report, at(0), quote)
                    .expect("검증")
                    .into_execution(at(seconds), observation(text, seconds))
                    .expect_err("소비 거부"),
                error
            );
        }
    }

    #[test]
    fn currency_range_construction_and_kr_market_are_checked() {
        let krw = Price::parse(&NumericInput::try_new("100").expect("입력"), Currency::Krw)
            .expect("가격");
        assert_eq!(range("100", "0.02", "4").check(krw), CheckResult::Failed);
        let facts = estimate_facts(
            instrument("TEST"),
            price("100"),
            ProposalVolatility::try_new(ratio("0.02")).expect("변동성"),
            TradingDayHorizon::try_new(ratio("4")).expect("기간"),
        );
        assert_eq!(
            ProposalPriceRange::from_estimate_task(
                facts.clone(),
                PriceObservation::from_quote_parts(instrument("OTHER"), price("100"), at(0))
                    .expect("관측")
            )
            .expect_err("종목"),
            ProposalError::InstrumentMismatch
        );
        assert_eq!(
            ProposalPriceRange::from_estimate_task(
                facts,
                PriceObservation::from_quote_parts(instrument("TEST"), krw, at(0)).expect("관측")
            )
            .expect_err("통화"),
            ProposalError::CurrencyMismatch
        );
        let original = draft();
        let wrong = ProposalPriceRange::from_estimate_task(
            estimate_facts(
                instrument("OTHER"),
                price("100"),
                ProposalVolatility::try_new(ratio("0.02")).expect("변동성"),
                TradingDayHorizon::try_new(ratio("4")).expect("기간"),
            ),
            PriceObservation::from_quote_parts(instrument("OTHER"), price("100"), at(0))
                .expect("관측"),
        )
        .expect("범위");
        assert_eq!(
            Proposal::new(
                original.id().clone(),
                original.intent().clone(),
                at(0),
                wrong,
                ProposalLifetime::default()
            )
            .expect_err("종목"),
            ProposalError::InstrumentMismatch
        );
    }
    #[test]
    fn kr_market_currency_and_lifetime_are_checked() {
        let original = draft();
        let krw = Price::parse(&NumericInput::try_new("100").expect("입력"), Currency::Krw)
            .expect("가격");
        let kr_instrument = ResolvedOrderInstrument::from_master_lookup_result(
            Symbol::try_new("005930").expect("종목"),
            Market::Kr,
        )
        .expect("시장");
        let terms = OrderTerms::KrMarket {
            side: crate::OrderSide::Buy,
            quantity: whole_quantity(),
        };
        let intent = OrderIntent::new(
            AccountId::new(7),
            kr_instrument.clone(),
            original.id().as_client_order_id().clone(),
            terms,
        )
        .expect("주문");
        let make_range = |base| {
            ProposalPriceRange::from_estimate_task(
                estimate_facts(
                    kr_instrument.clone(),
                    base,
                    ProposalVolatility::try_new(ratio("0.02")).expect("변동성"),
                    TradingDayHorizon::try_new(ratio("4")).expect("기간"),
                ),
                PriceObservation::from_quote_parts(kr_instrument.clone(), base, at(0))
                    .expect("관측"),
            )
            .expect("범위")
        };
        assert_eq!(
            Proposal::new(
                original.id().clone(),
                intent.clone(),
                at(0),
                make_range(price("100")),
                ProposalLifetime::default()
            )
            .expect_err("시장 통화"),
            ProposalError::CurrencyMismatch
        );
        let accepted = Proposal::new(
            original.id().clone(),
            intent,
            at(0),
            make_range(krw),
            ProposalLifetime::try_new(SpanSeconds::new(1)).expect("설정"),
        )
        .expect("KR 제안");
        assert_eq!(accepted.expires_at(), at(1));
        assert!(format!("{accepted:?}").contains("created_at"));
        let quote = observation("100", 0);
        assert_eq!(quote.observed_at(), at(0));
        assert_eq!(
            independent_range(100, 100, ratio("0.02"), ratio("4")),
            CheckResult::Passed
        );
    }
    #[test]
    fn price_difference_propagates_borrow_across_word_boundaries() {
        for (base, current, sigma) in [
            ("42949672.96", "42949672.95", "0.000000001"),
            ("42949672.95", "42949672.96", "0.000000001"),
            (
                "18446744073709551616",
                "18446744073709551615",
                "0.000000000000000001",
            ),
            (
                "18446744073709551615",
                "18446744073709551616",
                "0.000000000000000001",
            ),
            (
                "0.0000000000000000000000000001",
                "0.0000000000000000000000000002",
                "2",
            ),
        ] {
            assert_eq!(
                range(base, sigma, "1").check(price(current)),
                CheckResult::Passed
            );
        }
    }
    #[test]
    fn multiword_differences_match_independent_rational_reference() {
        let mut runner = TestRunner::new(Config::with_cases(4096));
        runner
            .run(
                &(64_u32..96, 1_u16..256, 1_u32..17, 0_u8..2),
                |(bits, delta, numerator, inverted)| {
                    let anchor = 1_i128.checked_shl(bits).expect("가격 계수");
                    let lower = anchor.checked_sub(i128::from(delta)).expect("낮은 가격");
                    let (base, current) = if inverted == 0 {
                        (anchor, lower)
                    } else {
                        (lower, anchor)
                    };
                    let sigma_text = format!("0.{numerator:018}");
                    let actual = range(&base.to_string(), &sigma_text, "1")
                        .check(price(&current.to_string()));
                    proptest::prop_assert_eq!(
                        actual,
                        independent_range(base, current, ratio(&sigma_text), ratio("1"))
                    );
                    Ok(())
                },
            )
            .expect("여러 자리 차이의 유리수 기준");
    }
    #[test]
    fn equal_timestamps_are_valid_for_consumption_and_recheck() {
        let proposal = draft();
        let quote = observation("100", 0);
        let report = passed(&proposal, at(0), &quote);
        let token = proposal
            .finish_validation(report, at(0), quote)
            .expect("검증")
            .into_execution(at(0), observation("100", 0))
            .expect("동일 시각 소비");
        let checked = token
            .recheck(at(0), observation("101", 0))
            .expect("동일 시각 재확인");
        assert_eq!(checked.checked_at(), at(0));
        assert_eq!(checked.validated_at(), at(0));
    }
    #[test]
    fn both_card_and_estimate_anchors_must_contain_the_current_price() {
        let separated = ProposalPriceRange::from_estimate_task(
            estimate_facts(
                instrument("TEST"),
                price("110"),
                ProposalVolatility::try_new(ratio("0.02")).expect("변동성"),
                TradingDayHorizon::try_new(ratio("4")).expect("기간"),
            ),
            observation("100", 0),
        )
        .expect("범위");
        assert_eq!(separated.check(price("100")), CheckResult::Failed);
        assert_eq!(separated.check(price("110")), CheckResult::Failed);
        let overlapping = ProposalPriceRange::from_estimate_task(
            estimate_facts(
                instrument("TEST"),
                price("101"),
                ProposalVolatility::try_new(ratio("0.02")).expect("변동성"),
                TradingDayHorizon::try_new(ratio("4")).expect("기간"),
            ),
            observation("100", 0),
        )
        .expect("범위");
        assert_eq!(overlapping.check(price("100")), CheckResult::Passed);
        assert_eq!(overlapping.check(price("102")), CheckResult::Passed);
        assert_eq!(overlapping.check(price("103")), CheckResult::Failed);
    }

    #[test]
    fn realistic_krw_prices_preserve_long_volatility_precision() {
        let instrument = ResolvedOrderInstrument::from_master_lookup_result(
            Symbol::try_new("005930").expect("종목"),
            Market::Kr,
        )
        .expect("시장");
        for (base, sigma, horizon) in [
            ("71234.5", "0.0213456789", "1"),
            ("1000000", "0.0123456789012345678901234567", "250"),
            ("71234.5", "0.1", "0.125"),
        ] {
            let reference =
                Price::parse(&NumericInput::try_new(base).expect("입력"), Currency::Krw)
                    .expect("가격");
            let facts = estimate_facts(
                instrument.clone(),
                reference,
                ProposalVolatility::try_new(ratio(sigma)).expect("변동성"),
                TradingDayHorizon::try_new(ratio(horizon)).expect("기간"),
            );
            let card = PriceObservation::from_quote_parts(instrument.clone(), reference, at(0))
                .expect("관측");
            assert_eq!(
                ProposalPriceRange::from_estimate_task(facts, card)
                    .expect("범위")
                    .check(reference),
                CheckResult::Passed
            );
        }
    }
}
