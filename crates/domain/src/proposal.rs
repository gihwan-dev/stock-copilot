use crate::{
    Currency, Market, Nanosecond, OrderIntent, Price, ProposalId, ProposalPriceRange, SpanSeconds,
    TimeError, TimeSpan, Timestamp,
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Draft;
#[derive(Debug)]
pub struct Validated {
    validated_at: Timestamp,
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
    Expiry,
    PriceRange,
}
impl ValidationCheck {
    pub const ALL: [Self; 8] = [
        Self::Instrument,
        Self::MarketHours,
        Self::AvailableFundsOrQuantity,
        Self::OppositePendingOrder,
        Self::BuyWarnings,
        Self::Principles,
        Self::Expiry,
        Self::PriceRange,
    ];
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationReport {
    gate_results: [CheckResult; 6],
    expiry: CheckResult,
    price_range: CheckResult,
}
impl ValidationReport {
    #[must_use]
    pub fn result(&self, check: ValidationCheck) -> CheckResult {
        let [
            instrument,
            market,
            available,
            opposite,
            warnings,
            principles,
        ] = self.gate_results;
        match check {
            ValidationCheck::Instrument => instrument,
            ValidationCheck::MarketHours => market,
            ValidationCheck::AvailableFundsOrQuantity => available,
            ValidationCheck::OppositePendingOrder => opposite,
            ValidationCheck::BuyWarnings => warnings,
            ValidationCheck::Principles => principles,
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
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalExecutionError {
    ClockRollback,
    Expired,
    PriceOutsideRange,
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
    ) -> Result<Self, ProposalError> {
        if id.as_client_order_id() != intent.client_order_id() {
            return Err(ProposalError::OrderKeyMismatch);
        }
        let currency = if intent.market() == Market::Kr {
            Currency::Krw
        } else {
            Currency::Usd
        };
        if range.currency()? != currency {
            return Err(ProposalError::CurrencyMismatch);
        }
        let expires_at = TimeSpan::try_new(SpanSeconds::new(600), Nanosecond::new(0))
            .and_then(|lifetime| created_at.checked_add(lifetime))
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
    pub fn report(&self, now: Timestamp, current: Price) -> ValidationReport {
        ValidationReport {
            gate_results: [CheckResult::Unknown; 6],
            expiry: self.expiry(now),
            price_range: self.range.check(current),
        }
    }
    pub fn validate(
        self,
        now: Timestamp,
        current: Price,
    ) -> Result<Proposal<Validated>, ValidationReport> {
        let report = self.report(now, current);
        self.finish_validation(report, now)
    }
    fn finish_validation(
        self,
        report: ValidationReport,
        now: Timestamp,
    ) -> Result<Proposal<Validated>, ValidationReport> {
        if ValidationCheck::ALL
            .into_iter()
            .any(|check| report.result(check) != CheckResult::Passed)
        {
            return Err(report);
        }
        Ok(Proposal {
            intent: self.intent,
            id: self.id,
            created_at: self.created_at,
            expires_at: self.expires_at,
            range: self.range,
            state: Validated { validated_at: now },
        })
    }
}
impl Proposal<Validated> {
    pub fn into_execution(
        self,
        now: Timestamp,
        current: Price,
    ) -> Result<OrderIntent, ProposalExecutionError> {
        if now < self.state.validated_at {
            return Err(ProposalExecutionError::ClockRollback);
        }
        if self.expiry(now) != CheckResult::Passed {
            return Err(ProposalExecutionError::Expired);
        }
        if self.range.check(current) != CheckResult::Passed {
            return Err(ProposalExecutionError::PriceOutsideRange);
        }
        Ok(self.intent)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CheckResult, Draft, Proposal, ProposalError, ProposalExecutionError, ValidationCheck,
        ValidationReport,
    };
    use crate::{
        AccountId, ClientOrderId, Currency, Market, Nanosecond, NumericInput, OffsetSeconds,
        OrderTerms, Price, ProposalId, ResolvedOrderInstrument, Symbol, Timestamp, UnixSeconds,
        WholeOrderQuantity,
    };
    use crate::{OrderIntent, ProposalPriceRange, ProposalVolatility, Ratio, TradingDayHorizon};
    use proptest::test_runner::{Config, TestRunner};

    fn at(seconds: i64, nanos: u32) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(nanos),
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
    fn draft() -> Proposal<Draft> {
        let intent = OrderIntent::new(
            AccountId::new(987_654_321),
            ResolvedOrderInstrument::from_master_lookup_result(
                Symbol::try_new("TEST").expect("종목"),
                Market::Us,
            )
            .expect("시장"),
            ClientOrderId::try_new("proposal-1").expect("키"),
            OrderTerms::UsMarketBuy {
                quantity: WholeOrderQuantity::try_new(
                    crate::Quantity::parse(&NumericInput::try_new("1").expect("입력"))
                        .expect("수량"),
                )
                .expect("주문 수량"),
            },
        )
        .expect("주문");
        Proposal::new(
            ProposalId::new(ClientOrderId::try_new("proposal-1").expect("제안 ID")),
            intent,
            at(0, 0),
            ProposalPriceRange::new(
                price("100"),
                price("100"),
                ProposalVolatility::try_new(ratio("0.02")).expect("변동성"),
                TradingDayHorizon::try_new(ratio("4")).expect("기간"),
            ),
        )
        .expect("제안")
    }
    fn cleared_report(
        proposal: &Proposal<Draft>,
        now: Timestamp,
        current: Price,
    ) -> ValidationReport {
        let mut report = proposal.report(now, current);
        report.gate_results = [CheckResult::Passed; 6];
        report
    }
    #[test]
    fn expiry_boundary_and_clock_rollback_are_closed() {
        let proposal = draft();
        for (now, expected) in [
            (at(0, 0), CheckResult::Passed),
            (at(599, 999_999_999), CheckResult::Passed),
            (at(600, 0), CheckResult::Failed),
            (at(-1, 0), CheckResult::Failed),
        ] {
            assert_eq!(
                proposal
                    .report(now, price("100"))
                    .result(ValidationCheck::Expiry),
                expected
            );
        }
        assert_eq!(proposal.expires_at(), at(600, 0));
    }
    #[test]
    fn pending_gate_never_validates_and_each_failure_blocks() {
        let pending = draft();
        let pending_report = pending.report(at(0, 0), price("100"));
        for check in ValidationCheck::ALL {
            assert_eq!(
                pending_report.result(check),
                if check == ValidationCheck::Expiry || check == ValidationCheck::PriceRange {
                    CheckResult::Passed
                } else {
                    CheckResult::Unknown
                }
            );
        }
        assert_eq!(
            pending
                .validate(at(0, 0), price("100"))
                .expect_err("미확인"),
            pending_report
        );
        for index in 0..6 {
            for result in [CheckResult::Failed, CheckResult::Unknown] {
                let proposal = draft();
                let mut report = cleared_report(&proposal, at(0, 0), price("100"));
                *report.gate_results.get_mut(index).expect("항목") = result;
                assert_eq!(
                    proposal
                        .finish_validation(report.clone(), at(0, 0))
                        .expect_err("검증 거부"),
                    report
                );
            }
        }
        for (now, current) in [(at(600, 0), price("100")), (at(0, 0), price("103"))] {
            let proposal = draft();
            let report = cleared_report(&proposal, now, current);
            assert_eq!(
                proposal
                    .finish_validation(report.clone(), now)
                    .expect_err("검증 거부"),
                report
            );
        }
    }
    #[test]
    fn validated_is_consumed_and_execution_rechecks_without_repricing() {
        for (now, current, expected) in [
            (at(599, 999_999_999), price("102"), Ok(())),
            (
                at(600, 0),
                price("100"),
                Err(ProposalExecutionError::Expired),
            ),
            (
                at(-1, 0),
                price("100"),
                Err(ProposalExecutionError::ClockRollback),
            ),
            (
                at(0, 0),
                price("103"),
                Err(ProposalExecutionError::PriceOutsideRange),
            ),
        ] {
            let proposal = draft();
            let original = proposal.intent().clone();
            assert_eq!(
                proposal.id().as_client_order_id(),
                original.client_order_id()
            );
            assert_eq!(proposal.created_at(), at(0, 0));
            assert!(!format!("{proposal:?}").contains("987654321"));
            let report = cleared_report(&proposal, at(0, 0), price("100"));
            let validated = proposal.finish_validation(report, at(0, 0)).expect("검증");
            assert_eq!(validated.intent(), &original);
            assert_eq!(
                validated.into_execution(now, current).map(|intent| {
                    assert_eq!(intent, original);
                }),
                expected
            );
        }
    }
    #[test]
    fn invalid_ranges_currency_time_and_arithmetic_fail_closed() {
        for text in ["0", "-0.01"] {
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
        let krw = Price::parse(&NumericInput::try_new("100").expect("입력"), Currency::Krw)
            .expect("가격");
        let mixed_range = ProposalPriceRange::new(
            krw,
            price("100"),
            ProposalVolatility::try_new(ratio("0.02")).expect("변동성"),
            TradingDayHorizon::try_new(ratio("4")).expect("기간"),
        );
        assert_eq!(mixed_range.currency(), Err(ProposalError::CurrencyMismatch));
        assert_eq!(
            Proposal::new(
                proposal.id().clone(),
                proposal.intent().clone(),
                at(0, 0),
                mixed_range
            )
            .expect_err("통화 불일치"),
            ProposalError::CurrencyMismatch
        );
        let range = ProposalPriceRange::new(
            krw,
            krw,
            ProposalVolatility::try_new(ratio("1")).expect("변동성"),
            TradingDayHorizon::try_new(ratio("1")).expect("기간"),
        );
        assert_eq!(
            Proposal::new(
                proposal.id().clone(),
                proposal.intent().clone(),
                at(0, 0),
                range
            )
            .expect_err("통화 불일치"),
            ProposalError::CurrencyMismatch
        );
        assert_eq!(
            Proposal::new(
                proposal.id().clone(),
                proposal.intent().clone(),
                at(253_402_300_799, 0),
                proposal.range
            )
            .expect_err("시간 넘침"),
            ProposalError::Time(crate::TimeError::Overflow)
        );
    }
    #[test]
    fn arithmetic_failure_and_either_anchor_fail_closed() {
        let proposal = draft();
        let krw = Price::parse(&NumericInput::try_new("100").expect("입력"), Currency::Krw)
            .expect("가격");
        assert_eq!(proposal.range.check(krw), CheckResult::Failed);
        for (reference, sigma, days, current) in [
            ("100", "0.00000000000001", "1", "100"),
            (
                "79228162514264337593543950335",
                "0.02",
                "4",
                "79228162514264337593543950335",
            ),
            ("100000000000000", "2", "100", "100000000000000"),
            ("79228162514264337593543950335", "1", "1", "1"),
            ("100", "0.0000000000000000000000000001", "1", "100"),
            ("100", "79228162514264337593543950335", "1", "100"),
            ("100", "1", "79228162514264337593543950335", "100"),
        ] {
            let failing_range = ProposalPriceRange::new(
                price(reference),
                price(reference),
                ProposalVolatility::try_new(ratio(sigma)).expect("변동성"),
                TradingDayHorizon::try_new(ratio(days)).expect("기간"),
            );
            assert_eq!(failing_range.check(price(current)), CheckResult::Failed);
        }
        let separated_range = ProposalPriceRange::new(
            price("100"),
            price("110"),
            ProposalVolatility::try_new(ratio("0.02")).expect("변동성"),
            TradingDayHorizon::try_new(ratio("4")).expect("기간"),
        );
        assert_eq!(separated_range.check(price("100")), CheckResult::Failed);
        assert_eq!(separated_range.check(price("110")), CheckResult::Failed);
    }

    #[test]
    fn proposal_identity_determines_key_and_krw_is_supported() {
        assert_eq!(
            ClientOrderId::try_new(" ").expect_err("잘못된 ID"),
            crate::ClientOrderIdError::InvalidFormat
        );
        let original = draft();
        assert_eq!(
            Proposal::new(
                ProposalId::new(ClientOrderId::try_new("other").expect("ID")),
                original.intent().clone(),
                at(0, 0),
                original.range
            )
            .expect_err("키 불일치"),
            ProposalError::OrderKeyMismatch
        );
        let id = ProposalId::new(ClientOrderId::try_new("kr-proposal").expect("ID"));
        let terms = OrderTerms::KrMarket {
            side: crate::OrderSide::Buy,
            quantity: WholeOrderQuantity::try_new(
                crate::Quantity::parse(&NumericInput::try_new("1").expect("입력")).expect("수량"),
            )
            .expect("주문 수량"),
        };
        let intent = OrderIntent::new(
            AccountId::new(7),
            ResolvedOrderInstrument::from_master_lookup_result(
                Symbol::try_new("005930").expect("종목"),
                Market::Kr,
            )
            .expect("시장"),
            id.as_client_order_id().clone(),
            terms,
        )
        .expect("주문");
        let krw = Price::parse(&NumericInput::try_new("100").expect("입력"), Currency::Krw)
            .expect("가격");
        let range = ProposalPriceRange::new(
            krw,
            krw,
            ProposalVolatility::try_new(ratio("0.02")).expect("변동성"),
            TradingDayHorizon::try_new(ratio("4")).expect("기간"),
        );
        let proposal = Proposal::new(id.clone(), intent, at(0, 0), range).expect("제안");
        let mut report = proposal.report(at(0, 0), krw);
        report.gate_results = [CheckResult::Passed; 6];
        let validated = proposal.finish_validation(report, at(0, 0)).expect("검증");
        assert_eq!(validated.id(), &id);
        let sent = validated.into_execution(at(1, 0), krw).expect("소비");
        assert_eq!(sent.client_order_id(), id.as_client_order_id());
        assert_eq!(sent.terms(), &terms);
    }

    #[test]
    fn squared_price_range_has_exact_inclusive_boundaries() {
        let proposal = draft();
        for (current, result) in [
            ("98", CheckResult::Passed),
            ("102", CheckResult::Passed),
            ("97.99999999", CheckResult::Failed),
            ("102.00000001", CheckResult::Failed),
        ] {
            assert_eq!(
                proposal
                    .report(at(0, 0), price(current))
                    .result(ValidationCheck::PriceRange),
                result
            );
        }
        let mut runner = TestRunner::new(Config::with_cases(4096));
        runner
            .run(
                &(1_i64..10000, 1_i64..100, 1_i64..100, 1_i64..10000),
                |(base, sigma, days, current)| {
                    let range = ProposalPriceRange::new(
                        price(&base.to_string()),
                        price(&base.to_string()),
                        ProposalVolatility::try_new(ratio(&format!("0.{sigma:03}")))
                            .expect("변동성"),
                        TradingDayHorizon::try_new(ratio(&format!("0.{days:02}"))).expect("기간"),
                    );
                    let difference = i128::from(current)
                        .checked_sub(i128::from(base))
                        .expect("차이");
                    let left = difference
                        .checked_pow(2)
                        .and_then(|value| value.checked_mul(400_000_000))
                        .expect("좌변");
                    let right = i128::from(sigma)
                        .checked_pow(2)
                        .and_then(|value| value.checked_mul(i128::from(days)))
                        .map(|value| {
                            value
                                .checked_mul(i128::from(base).checked_pow(2).expect("제곱"))
                                .expect("정수 곱")
                        })
                        .expect("우변");
                    proptest::prop_assert_eq!(
                        range.check(price(&current.to_string())),
                        if left <= right {
                            CheckResult::Passed
                        } else {
                            CheckResult::Failed
                        }
                    );
                    Ok(())
                },
            )
            .expect("독립 정수 기준");
    }
}
