use crate::{
    AccountDisplayFact, ClientOrderPrefix, DisplayInstrumentName, EstimatedOrderFee,
    ExecutionToken, InstrumentNameFact, Market, MaskedAccountNumber, MoneyError, OrderAmountBasis,
    OrderExpectedAmount, OrderFxRates, OrderSide, OrderTerms, Price, Quantity, Symbol,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummaryQuantity {
    Specified(Quantity),
    Absent,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummaryLimitPrice {
    Specified(Price),
    Absent,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSummaryError {
    InstrumentMismatch,
    AccountMismatch,
    FeeOrderMismatch,
    FeeCurrencyMismatch,
    NegativeFee,
    Arithmetic(MoneyError),
}
#[derive(Debug, PartialEq, Eq)]
pub struct OrderSummary {
    instrument_name: DisplayInstrumentName,
    symbol: Symbol,
    market: Market,
    side: OrderSide,
    quantity: SummaryQuantity,
    limit_price: SummaryLimitPrice,
    expected_amount: OrderExpectedAmount,
    expected_fee: EstimatedOrderFee,
    account_suffix: MaskedAccountNumber,
    client_order_prefix: ClientOrderPrefix,
}
#[derive(Debug)]
pub struct PendingOrderConfirmation {
    token: ExecutionToken,
    summary: OrderSummary,
}
impl PendingOrderConfirmation {
    pub fn from_execution(
        token: ExecutionToken,
        instrument: InstrumentNameFact,
        account: AccountDisplayFact,
        fee: EstimatedOrderFee,
        rates: OrderFxRates,
    ) -> Result<Self, OrderSummaryError> {
        let summary = OrderSummary::build(&token, instrument, account, fee, rates)?;
        Ok(Self { token, summary })
    }
    #[must_use]
    pub fn summary(&self) -> &OrderSummary {
        &self.summary
    }
    pub fn recheck(
        self,
        now: crate::Timestamp,
        observation: crate::PriceObservation,
    ) -> Result<Self, crate::ProposalExecutionError> {
        let (token, summary) = self.into_execution_parts();
        let checked = token.recheck(now, observation)?;
        Ok(Self {
            token: checked,
            summary,
        })
    }
    pub(crate) fn into_execution_parts(self) -> (ExecutionToken, OrderSummary) {
        (self.token, self.summary)
    }
}

impl OrderSummary {
    fn build(
        token: &ExecutionToken,
        instrument: InstrumentNameFact,
        account: AccountDisplayFact,
        fee: EstimatedOrderFee,
        rates: OrderFxRates,
    ) -> Result<Self, OrderSummaryError> {
        let intent = token.intent();
        let (named_instrument, instrument_name) = instrument.into_parts();
        if &named_instrument != intent.instrument() {
            return Err(OrderSummaryError::InstrumentMismatch);
        }
        let (account_id, account_suffix) = account.into_parts();
        if &account_id != intent.account_id() {
            return Err(OrderSummaryError::AccountMismatch);
        }
        let (quantity, limit_price, money, basis) = order_values(token)?;
        if let EstimatedOrderFee::Confirmed(confirmed) = &fee {
            if &confirmed.0 != intent.client_order_id() {
                return Err(OrderSummaryError::FeeOrderMismatch);
            }
            if confirmed.money().currency() != money.currency() {
                return Err(OrderSummaryError::FeeCurrencyMismatch);
            }
            if confirmed.money().amount().as_decimal().is_sign_negative() {
                return Err(OrderSummaryError::NegativeFee);
            }
        }
        Ok(Self {
            instrument_name,
            symbol: intent.symbol().clone(),
            market: intent.market(),
            side: intent.side(),
            quantity,
            limit_price,
            expected_amount: OrderExpectedAmount::from_order(
                money,
                basis,
                rates,
                token.checked_at(),
            ),
            expected_fee: fee,
            account_suffix,
            client_order_prefix: ClientOrderPrefix::from_order_id(intent.client_order_id()),
        })
    }
    #[must_use]
    pub fn instrument_name(&self) -> &DisplayInstrumentName {
        &self.instrument_name
    }
    #[must_use]
    pub fn symbol(&self) -> &Symbol {
        &self.symbol
    }
    #[must_use]
    pub fn market(&self) -> Market {
        self.market.clone()
    }
    #[must_use]
    pub fn side(&self) -> OrderSide {
        self.side
    }
    #[must_use]
    pub fn quantity(&self) -> SummaryQuantity {
        self.quantity
    }
    #[must_use]
    pub fn limit_price(&self) -> SummaryLimitPrice {
        self.limit_price
    }
    #[must_use]
    pub fn expected_amount(&self) -> &OrderExpectedAmount {
        &self.expected_amount
    }
    #[must_use]
    pub fn expected_fee(&self) -> &EstimatedOrderFee {
        &self.expected_fee
    }
    #[must_use]
    pub fn account_suffix(&self) -> &MaskedAccountNumber {
        &self.account_suffix
    }
    #[must_use]
    pub fn client_order_prefix(&self) -> &ClientOrderPrefix {
        &self.client_order_prefix
    }
}
fn order_values(
    token: &ExecutionToken,
) -> Result<
    (
        SummaryQuantity,
        SummaryLimitPrice,
        crate::Money,
        OrderAmountBasis,
    ),
    OrderSummaryError,
> {
    let intent = token.intent();
    let (quantity, limit_price, result, basis) = match *intent.terms() {
        OrderTerms::KrLimit {
            quantity, price, ..
        } => {
            let limit = price.price();
            (
                SummaryQuantity::Specified(quantity.quantity()),
                SummaryLimitPrice::Specified(limit),
                limit.checked_mul(&quantity.quantity()),
                OrderAmountBasis::Fixed,
            )
        }
        OrderTerms::UsLimit {
            quantity, price, ..
        } => {
            let limit = price.price();
            (
                SummaryQuantity::Specified(quantity.quantity()),
                SummaryLimitPrice::Specified(limit),
                limit.checked_mul(&quantity.quantity()),
                OrderAmountBasis::Fixed,
            )
        }
        OrderTerms::KrMarket { quantity, .. } | OrderTerms::UsMarketBuy { quantity } => (
            SummaryQuantity::Specified(quantity.quantity()),
            SummaryLimitPrice::Absent,
            token
                .execution_observation()
                .price()
                .checked_mul(&quantity.quantity()),
            OrderAmountBasis::Estimated,
        ),
        OrderTerms::UsMarketSell { quantity } => (
            SummaryQuantity::Specified(quantity.quantity()),
            SummaryLimitPrice::Absent,
            token
                .execution_observation()
                .price()
                .checked_mul(&quantity.quantity()),
            OrderAmountBasis::Estimated,
        ),
        OrderTerms::UsMarketAmountBuy { amount } => (
            SummaryQuantity::Absent,
            SummaryLimitPrice::Absent,
            Ok(amount.money()),
            OrderAmountBasis::Fixed,
        ),
    };
    Ok((
        quantity,
        limit_price,
        result.map_err(OrderSummaryError::Arithmetic)?,
        basis,
    ))
}

#[cfg(test)]
mod field_tests {
    use super::{EstimatedOrderFee, Market, OrderSide, OrderSummary};
    use crate::LargeOrderConfirmation;
    #[test]
    fn summary_fields_match_architecture_without_rest_pattern() {
        let token = crate::order_summary::tests::field_test_token();
        let summary = crate::order_summary::tests::field_test_summary(&token);
        let OrderSummary {
            instrument_name,
            symbol,
            market,
            side,
            quantity,
            limit_price,
            expected_amount,
            expected_fee,
            account_suffix,
            client_order_prefix,
        } = summary;
        assert_eq!(instrument_name.as_str(), "테스트 종목");
        assert_eq!(&symbol, token.intent().symbol());
        assert_eq!(market, Market::Kr);
        assert_eq!(side, OrderSide::Buy);
        assert_eq!(
            quantity,
            crate::SummaryQuantity::Specified(
                crate::Quantity::parse(&crate::NumericInput::try_new("1").expect("입력"))
                    .expect("수량")
            )
        );
        assert_eq!(
            limit_price,
            crate::SummaryLimitPrice::Specified(
                crate::Price::parse(
                    &crate::NumericInput::try_new("100").expect("입력"),
                    crate::Currency::Krw
                )
                .expect("가격")
            )
        );
        assert_eq!(
            expected_amount.confirmation(),
            LargeOrderConfirmation::NotRequired
        );
        assert_eq!(expected_fee, EstimatedOrderFee::Unknown);
        assert_eq!(account_suffix.to_string(), "1234");
        assert_eq!(client_order_prefix.as_str(), "abcdefgh");
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::PendingOrderConfirmation;
    use crate::{
        AccountDisplayFact, AccountId, AccountLookupDisplayRow, ClientOrderId, ConfirmedOrderFee,
        Currency, DisplayInstrumentName, DisplayInstrumentNameError, EstimatedOrderFee,
        ExecutionToken, FxConversionError, FxPair, FxRate, FxRateKind, FxSource,
        InstrumentNameFact, KrOrderPrice, LargeOrderConfirmation, Market, MaskedAccountNumber,
        Money, MoneyError, Nanosecond, NumericInput, OffsetSeconds, OrderAmountBasis, OrderFxRates,
        OrderFxUnavailable, OrderIntent, OrderKrwEstimate, OrderSide, OrderSummary,
        OrderSummaryError, OrderTerms, Price, Quantity, ResolvedOrderInstrument, SellOrderQuantity,
        SummaryLimitPrice, SummaryQuantity, Symbol, TimeInterval, Timestamp, UnixSeconds,
        UsOrderPrice, UsdOrderAmount, WholeOrderQuantity,
    };
    use proptest::prop_assert_eq;
    use proptest::test_runner::{Config, TestRunner};

    fn money(value: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(value).expect("입력"), currency).expect("금액")
    }
    fn price(value: &str, currency: Currency) -> Price {
        Price::try_new(money(value, currency)).expect("가격")
    }
    fn quantity(value: &str) -> WholeOrderQuantity {
        WholeOrderQuantity::try_new(
            Quantity::parse(&NumericInput::try_new(value).expect("입력")).expect("수량"),
        )
        .expect("정수")
    }
    fn instrument(market: Market) -> ResolvedOrderInstrument {
        ResolvedOrderInstrument::from_master_lookup_result(
            Symbol::try_new("TEST").expect("코드"),
            market,
        )
        .expect("시장")
    }
    fn token(terms: OrderTerms, observed: &str) -> ExecutionToken {
        let intent = OrderIntent::new(
            AccountId::new(987_654_321),
            instrument(terms.market()),
            ClientOrderId::try_new("abcdefgh-extra").expect("키"),
            terms,
        )
        .expect("주문");
        let quote = price(
            observed,
            match terms {
                OrderTerms::KrLimit { .. } | OrderTerms::KrMarket { .. } => Currency::Krw,
                OrderTerms::UsLimit { .. }
                | OrderTerms::UsMarketBuy { .. }
                | OrderTerms::UsMarketSell { .. }
                | OrderTerms::UsMarketAmountBuy { .. } => Currency::Usd,
            },
        );
        crate::proposal::summary_tests::token(intent, quote)
    }
    fn summary(
        token: &ExecutionToken,
        rates: OrderFxRates,
    ) -> Result<OrderSummary, OrderSummaryError> {
        OrderSummary::build(
            token,
            InstrumentNameFact::from_master_lookup_result(
                token.intent().instrument().clone(),
                test_name(),
            ),
            AccountDisplayFact::from_account_lookup_result(
                AccountLookupDisplayRow::from_broker_account_result(
                    token.intent().account_id().clone(),
                    test_suffix(),
                ),
            ),
            EstimatedOrderFee::Unknown,
            rates,
        )
    }
    fn rate(value: &str, kind: FxRateKind) -> FxRate {
        FxRate::parse(
            &NumericInput::try_new(value).expect("입력"),
            FxPair::UsdKrw,
            kind,
            TimeInterval::try_new(at(0), at(10)).expect("기간"),
            FxSource::TossOpenApi,
        )
        .expect("환율")
    }
    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn kr_limit(value: &str, side: OrderSide) -> ExecutionToken {
        token(
            OrderTerms::KrLimit {
                side,
                quantity: quantity("1"),
                price: KrOrderPrice::try_new(price(value, Currency::Krw)).expect("지정가"),
            },
            "100",
        )
    }
    #[test]
    fn boundary_includes_sells_and_does_not_reject_orders_over_one_hundred_million() {
        for side in [OrderSide::Buy, OrderSide::Sell] {
            for (value, expected) in [
                ("79999999", LargeOrderConfirmation::NotRequired),
                ("80000000", LargeOrderConfirmation::Required),
                ("100000001", LargeOrderConfirmation::Required),
            ] {
                let token = kr_limit(value, side);
                let result = summary(&token, OrderFxRates::missing()).expect("요약");
                assert_eq!(result.expected_amount().confirmation(), expected);
                assert_eq!(
                    result.expected_amount().money(),
                    money(value, Currency::Krw)
                );
                assert_eq!(result.expected_fee(), &EstimatedOrderFee::Unknown);
                assert_eq!(
                    result.quantity(),
                    SummaryQuantity::Specified(quantity("1").quantity())
                );
                assert_eq!(
                    result.limit_price(),
                    SummaryLimitPrice::Specified(price(value, Currency::Krw))
                );
                assert_eq!(result.expected_amount().basis(), OrderAmountBasis::Fixed);
                assert_eq!(result.symbol(), token.intent().symbol());
                assert_eq!(result.market(), Market::Kr);
                assert_eq!(result.side(), side);
                assert_eq!(result.instrument_name().as_str(), "테스트 종목");
                assert_eq!(result.account_suffix().to_string(), "1234");
                assert_eq!(result.client_order_prefix().as_str(), "abcdefgh");
                assert!(!format!("{result:?}").contains("987654321"));
                assert!(!format!("{result:?}").contains("123456781234"));
            }
        }
    }
    #[test]
    fn threshold_matches_independent_integer_oracle() {
        TestRunner::new(Config::with_cases(4096))
            .run(&(0_u64..200_000_000), |offset| {
                let amount = offset + 1;
                let token = kr_limit(&amount.to_string(), OrderSide::Buy);
                let result = summary(&token, OrderFxRates::missing()).expect("요약");
                prop_assert_eq!(
                    result.expected_amount().confirmation(),
                    if amount >= 80_000_000 {
                        LargeOrderConfirmation::Required
                    } else {
                        LargeOrderConfirmation::NotRequired
                    }
                );
                Ok(())
            })
            .expect("독립 정수 기준");
    }
    #[test]
    fn usd_uses_larger_of_both_rates_and_keeps_approximate_provenance() {
        for (buy, mid, selected_kind) in [
            ("1600", "1500", FxRateKind::Buy),
            ("1500", "1600", FxRateKind::Mid),
            ("1600", "1600", FxRateKind::Buy),
        ] {
            let token = token(
                OrderTerms::UsMarketAmountBuy {
                    amount: UsdOrderAmount::try_new(money("50000", Currency::Usd)).expect("금액"),
                },
                "100",
            );
            let result = summary(
                &token,
                OrderFxRates::from_broker_fx_result(
                    rate(buy, FxRateKind::Buy),
                    rate(mid, FxRateKind::Mid),
                ),
            )
            .expect("요약");
            assert_eq!(result.quantity(), SummaryQuantity::Absent);
            assert_eq!(result.limit_price(), SummaryLimitPrice::Absent);
            assert_eq!(
                result.expected_amount().money(),
                money("50000", Currency::Usd)
            );
            assert_eq!(
                result.expected_amount().confirmation(),
                LargeOrderConfirmation::Required
            );
            assert_eq!(
                result.expected_amount().krw_estimate(),
                OrderKrwEstimate::Converted(crate::ConvertedMoney::from_conversion(
                    money("80000000", Currency::Krw),
                    rate("1600", selected_kind)
                ))
            );
        }
    }
    pub(crate) fn field_test_token() -> ExecutionToken {
        kr_limit("100", OrderSide::Buy)
    }
    pub(crate) fn field_test_summary(token: &ExecutionToken) -> OrderSummary {
        summary(token, OrderFxRates::missing()).expect("요약")
    }

    #[test]
    fn all_market_quantities_are_estimated_and_limit_totals_are_exact() {
        let whole = quantity("3");
        for terms in [
            OrderTerms::KrMarket {
                side: OrderSide::Sell,
                quantity: whole,
            },
            OrderTerms::UsMarketBuy { quantity: whole },
            OrderTerms::UsMarketSell {
                quantity: SellOrderQuantity::try_new(
                    Quantity::parse(&NumericInput::try_new("0.125").expect("입력")).expect("수량"),
                )
                .expect("매도"),
            },
        ] {
            let token = token(terms, "123.456");
            let result = summary(&token, OrderFxRates::missing()).expect("요약");
            assert_eq!(
                result.expected_amount().basis(),
                OrderAmountBasis::Estimated
            );
            assert_eq!(result.limit_price(), SummaryLimitPrice::Absent);
            let expected = match terms {
                OrderTerms::UsMarketSell { .. } => "15.432",
                OrderTerms::KrLimit { .. }
                | OrderTerms::KrMarket { .. }
                | OrderTerms::UsLimit { .. }
                | OrderTerms::UsMarketBuy { .. }
                | OrderTerms::UsMarketAmountBuy { .. } => "370.368",
            };
            assert_eq!(
                result.expected_amount().money(),
                money(
                    expected,
                    token.execution_observation().price().as_money().currency()
                )
            );
        }
        let token = token(
            OrderTerms::UsLimit {
                side: OrderSide::Sell,
                quantity: whole,
                price: UsOrderPrice::try_new(price("12.34", Currency::Usd)).expect("가격"),
            },
            "99",
        );
        let result = summary(&token, OrderFxRates::missing()).expect("요약");
        assert_eq!(
            result.expected_amount().money(),
            money("37.02", Currency::Usd)
        );
        assert_eq!(result.expected_amount().basis(), OrderAmountBasis::Fixed);
        assert_eq!(
            result.quantity(),
            SummaryQuantity::Specified(whole.quantity())
        );
        assert_eq!(
            result.limit_price(),
            SummaryLimitPrice::Specified(price("12.34", Currency::Usd))
        );
    }
    #[test]
    fn mismatched_facts_and_fees_produce_no_summary() {
        let token = field_test_token();
        let name = || {
            InstrumentNameFact::from_master_lookup_result(
                token.intent().instrument().clone(),
                DisplayInstrumentName::try_new("이름").expect("이름"),
            )
        };
        let account = || {
            AccountDisplayFact::from_account_lookup_result(
                AccountLookupDisplayRow::from_broker_account_result(
                    token.intent().account_id().clone(),
                    MaskedAccountNumber::try_new("123456789012").expect("번호"),
                ),
            )
        };
        for wrong in [
            ResolvedOrderInstrument::from_master_lookup_result(
                Symbol::try_new("OTHER").expect("코드"),
                Market::Kr,
            )
            .expect("종목"),
            instrument(Market::Us),
        ] {
            assert_eq!(
                OrderSummary::build(
                    &token,
                    InstrumentNameFact::from_master_lookup_result(
                        wrong,
                        DisplayInstrumentName::try_new("이름").expect("이름")
                    ),
                    account(),
                    EstimatedOrderFee::Unknown,
                    OrderFxRates::missing()
                ),
                Err(OrderSummaryError::InstrumentMismatch)
            );
        }
        let wrong_account = AccountDisplayFact::from_account_lookup_result(
            AccountLookupDisplayRow::from_broker_account_result(
                AccountId::new(123),
                MaskedAccountNumber::try_new("123456789012").expect("번호"),
            ),
        );
        assert_eq!(
            OrderSummary::build(
                &token,
                name(),
                wrong_account,
                EstimatedOrderFee::Unknown,
                OrderFxRates::missing()
            ),
            Err(OrderSummaryError::AccountMismatch)
        );
        let debug_account = format!("{:?}", account());
        assert!(!debug_account.contains("987654321"));
        assert!(debug_account.contains("9012"));
        assert!(debug_account.contains("AccountDisplayFact"));
    }
    #[test]
    fn confirmed_fee_checks_order_currency_sign_and_excludes_cost_from_threshold() {
        let token = field_test_token();
        let name = || {
            InstrumentNameFact::from_master_lookup_result(
                token.intent().instrument().clone(),
                test_name(),
            )
        };
        let account = || {
            AccountDisplayFact::from_account_lookup_result(
                AccountLookupDisplayRow::from_broker_account_result(
                    token.intent().account_id().clone(),
                    test_suffix(),
                ),
            )
        };
        for (id, amount, currency, expected) in [
            (
                "wrong",
                "1",
                Currency::Krw,
                Some(OrderSummaryError::FeeOrderMismatch),
            ),
            (
                "abcdefgh-extra",
                "1",
                Currency::Usd,
                Some(OrderSummaryError::FeeCurrencyMismatch),
            ),
            (
                "abcdefgh-extra",
                "-1",
                Currency::Krw,
                Some(OrderSummaryError::NegativeFee),
            ),
            ("abcdefgh-extra", "80000000", Currency::Krw, None),
            ("abcdefgh-extra", "0", Currency::Krw, None),
        ] {
            let fee = EstimatedOrderFee::Confirmed(ConfirmedOrderFee(
                ClientOrderId::try_new(id).expect("키"),
                money(amount, currency),
            ));
            let result = OrderSummary::build(
                &token,
                name(),
                account(),
                fee.clone(),
                OrderFxRates::missing(),
            );
            if let Some(error) = expected {
                assert_eq!(result, Err(error));
            } else {
                let checked = result.expect("요약");
                assert_eq!(checked.expected_fee(), &fee);
                assert_eq!(
                    checked.expected_amount().confirmation(),
                    LargeOrderConfirmation::NotRequired
                );
            }
        }
    }
    #[test]
    fn names_are_bounded_control_free_and_short_keys_are_not_padded() {
        for invalid in [
            String::new(),
            "a".repeat(257),
            "이름\n".to_owned(),
            "a\0b".to_owned(),
            "a\u{7f}".to_owned(),
        ] {
            assert_eq!(
                DisplayInstrumentName::try_new(&invalid),
                Err(DisplayInstrumentNameError::InvalidName)
            );
        }
        for valid in ["a".repeat(256), "한글 이름".to_owned()] {
            assert_eq!(
                DisplayInstrumentName::try_new(&valid)
                    .expect("이름")
                    .as_str(),
                valid
            );
        }
        let id = ClientOrderId::try_new("a").expect("짧은 키");
        let intent = OrderIntent::new(
            AccountId::new(1),
            instrument(Market::Kr),
            id,
            OrderTerms::KrMarket {
                side: OrderSide::Buy,
                quantity: quantity("1"),
            },
        )
        .expect("의도");
        let token = crate::proposal::summary_tests::token(intent, price("1", Currency::Krw));
        assert_eq!(
            summary(&token, OrderFxRates::missing())
                .expect("요약")
                .client_order_prefix()
                .as_str(),
            "a"
        );
    }
    #[test]
    fn multiplication_overflow_and_precision_loss_block_summary() {
        let max = "79228162514264337593543950335";
        for (terms, observed, error) in [
            (
                OrderTerms::KrLimit {
                    side: OrderSide::Buy,
                    quantity: quantity(max),
                    price: KrOrderPrice::try_new(price("2", Currency::Krw)).expect("가격"),
                },
                "1",
                MoneyError::Overflow,
            ),
            (
                OrderTerms::UsMarketSell {
                    quantity: SellOrderQuantity::try_new(
                        Quantity::parse(&NumericInput::try_new("0.000001").expect("입력"))
                            .expect("수량"),
                    )
                    .expect("매도"),
                },
                "0.0000000000000000000000000001",
                MoneyError::PrecisionLoss,
            ),
        ] {
            assert_eq!(
                summary(&token(terms, observed), OrderFxRates::missing()),
                Err(OrderSummaryError::Arithmetic(error))
            );
        }
    }
    #[test]
    fn unusable_fx_always_requires_confirmation_and_preserves_failure() {
        let token = token(
            OrderTerms::UsMarketAmountBuy {
                amount: UsdOrderAmount::try_new(money("1", Currency::Usd)).expect("금액"),
            },
            "100",
        );
        let valid_buy = rate("1500", FxRateKind::Buy);
        let valid_mid = rate("1400", FxRateKind::Mid);
        let reverse_mid = FxRate::parse(
            &NumericInput::try_new("1").expect("입력"),
            FxPair::KrwUsd,
            FxRateKind::Mid,
            TimeInterval::try_new(at(0), at(10)).expect("기간"),
            FxSource::TossOpenApi,
        )
        .expect("환율");
        for (buy, mid) in [
            (valid_buy, reverse_mid),
            (valid_mid, valid_mid),
            (valid_buy, valid_buy),
            (
                FxRate::parse(
                    &NumericInput::try_new("1").expect("입력"),
                    FxPair::KrwUsd,
                    FxRateKind::Buy,
                    TimeInterval::try_new(at(0), at(10)).expect("기간"),
                    FxSource::TossOpenApi,
                )
                .expect("환율"),
                valid_mid,
            ),
        ] {
            let result =
                summary(&token, OrderFxRates::from_broker_fx_result(buy, mid)).expect("요약");
            assert_eq!(
                result.expected_amount().krw_estimate(),
                OrderKrwEstimate::Unavailable(OrderFxUnavailable::InvalidRates)
            );
            assert_eq!(
                result.expected_amount().confirmation(),
                LargeOrderConfirmation::Required
            );
        }
    }
    #[test]
    fn fx_validity_checks_both_intervals_at_token_time() {
        let token = token_amount("1");
        let valid_buy = rate("1500", FxRateKind::Buy);
        let valid_mid = rate("1400", FxRateKind::Mid);
        for (start, end, valid) in [(1, 2, true), (0, 1, false), (2, 3, false)] {
            let interval = TimeInterval::try_new(at(start), at(end)).expect("기간");
            for stale_slot in [FxRateKind::Buy, FxRateKind::Mid] {
                let stale = FxRate::parse(
                    &NumericInput::try_new(match stale_slot {
                        FxRateKind::Buy => "1300",
                        FxRateKind::Mid => "1400",
                    })
                    .expect("입력"),
                    FxPair::UsdKrw,
                    stale_slot,
                    interval,
                    FxSource::TossOpenApi,
                )
                .expect("환율");
                let (buy, mid) = match stale_slot {
                    FxRateKind::Buy => (stale, valid_mid),
                    FxRateKind::Mid => (valid_buy, stale),
                };
                let result =
                    summary(&token, OrderFxRates::from_broker_fx_result(buy, mid)).expect("요약");
                assert_eq!(
                    result.expected_amount().confirmation(),
                    if valid {
                        LargeOrderConfirmation::NotRequired
                    } else {
                        LargeOrderConfirmation::Required
                    }
                );
            }
        }
    }
    #[test]
    fn missing_and_overflowing_fx_are_required() {
        let token = token_amount("1");
        let missing = summary(&token, OrderFxRates::missing()).expect("요약");
        assert_eq!(
            missing.expected_amount().krw_estimate(),
            OrderKrwEstimate::Unavailable(OrderFxUnavailable::Missing)
        );
        assert_eq!(
            missing.expected_amount().confirmation(),
            LargeOrderConfirmation::Required
        );
        let huge = token_amount("79228162514264337593543950335");
        let overflow = summary(
            &huge,
            OrderFxRates::from_broker_fx_result(
                rate("500", FxRateKind::Buy),
                rate("500", FxRateKind::Mid),
            ),
        )
        .expect("요약");
        assert_eq!(
            overflow.expected_amount().krw_estimate(),
            OrderKrwEstimate::Unavailable(OrderFxUnavailable::Conversion(
                FxConversionError::Arithmetic(MoneyError::Overflow)
            ))
        );
    }
    fn token_amount(value: &str) -> ExecutionToken {
        token(
            OrderTerms::UsMarketAmountBuy {
                amount: UsdOrderAmount::try_new(money(value, Currency::Usd)).expect("금액"),
            },
            "1",
        )
    }
    #[test]
    fn usd_threshold_matches_independent_integer_product() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(1_u64..100_000, 500_u64..3000, 500_u64..3000),
                |(dollars, buy, mid)| {
                    let token = token_amount(&dollars.to_string());
                    let result = summary(
                        &token,
                        OrderFxRates::from_broker_fx_result(
                            rate(&buy.to_string(), FxRateKind::Buy),
                            rate(&mid.to_string(), FxRateKind::Mid),
                        ),
                    )
                    .expect("요약");
                    prop_assert_eq!(
                        result.expected_amount().confirmation(),
                        if buy.max(mid) * 100 > buy.min(mid) * 120
                            || dollars * buy.max(mid) >= 80_000_000
                        {
                            LargeOrderConfirmation::Required
                        } else {
                            LargeOrderConfirmation::NotRequired
                        }
                    );
                    Ok(())
                },
            )
            .expect("독립 정수 곱");
    }

    fn test_name() -> DisplayInstrumentName {
        DisplayInstrumentName::try_new("테스트 종목").expect("이름")
    }
    fn test_suffix() -> MaskedAccountNumber {
        MaskedAccountNumber::try_new("123456781234").expect("가림")
    }
    #[test]
    fn indeterminate_fx_never_falls_through_to_not_required() {
        TestRunner::new(Config::with_cases(4096))
            .run(&(1_u64..100_000, 0_u8..3), |(dollars, failure)| {
                let token = token_amount(&dollars.to_string());
                let invalid = rate("1", FxRateKind::Mid);
                let rates = match failure {
                    0 => OrderFxRates::missing(),
                    1 => OrderFxRates::from_broker_fx_result(invalid, invalid),
                    _ => {
                        let future = FxRate::parse(
                            &NumericInput::try_new("1").expect("입력"),
                            FxPair::UsdKrw,
                            FxRateKind::Buy,
                            TimeInterval::try_new(at(2), at(3)).expect("기간"),
                            FxSource::TossOpenApi,
                        )
                        .expect("환율");
                        OrderFxRates::from_broker_fx_result(future, invalid)
                    }
                };
                let result = summary(&token, rates).expect("요약");
                prop_assert_eq!(
                    result.expected_amount().confirmation(),
                    LargeOrderConfirmation::Required
                );
                Ok(())
            })
            .expect("판정 불가 안전 경로");
    }
    #[test]
    fn fractional_krw_conversion_rounds_up_only_for_confirmation() {
        for (fx, expected) in [
            ("799.99999", LargeOrderConfirmation::NotRequired),
            ("799.9999999", LargeOrderConfirmation::Required),
            ("800", LargeOrderConfirmation::Required),
        ] {
            let token = token_amount("100000");
            let result = summary(
                &token,
                OrderFxRates::from_broker_fx_result(
                    rate(fx, FxRateKind::Buy),
                    rate(fx, FxRateKind::Mid),
                ),
            )
            .expect("요약");
            assert_eq!(result.expected_amount().confirmation(), expected);
        }
    }

    struct SummaryCase {
        terms: OrderTerms,
        market: Market,
        side: OrderSide,
        quantity: SummaryQuantity,
        limit: SummaryLimitPrice,
        money: Money,
    }
    fn kr_field_cases() -> [SummaryCase; 2] {
        let whole = quantity("3");
        [
            SummaryCase {
                terms: OrderTerms::KrLimit {
                    side: OrderSide::Buy,
                    quantity: whole,
                    price: KrOrderPrice::try_new(price("1000", Currency::Krw)).expect("가격"),
                },
                market: Market::Kr,
                side: OrderSide::Buy,
                quantity: SummaryQuantity::Specified(whole.quantity()),
                limit: SummaryLimitPrice::Specified(price("1000", Currency::Krw)),
                money: money("3000", Currency::Krw),
            },
            SummaryCase {
                terms: OrderTerms::KrMarket {
                    side: OrderSide::Sell,
                    quantity: whole,
                },
                market: Market::Kr,
                side: OrderSide::Sell,
                quantity: SummaryQuantity::Specified(whole.quantity()),
                limit: SummaryLimitPrice::Absent,
                money: money("370.368", Currency::Krw),
            },
        ]
    }
    fn us_field_cases() -> [SummaryCase; 4] {
        let whole = quantity("3");
        let fractional = SellOrderQuantity::try_new(
            Quantity::parse(&NumericInput::try_new("0.125").expect("입력")).expect("수량"),
        )
        .expect("매도");
        [
            SummaryCase {
                terms: OrderTerms::UsLimit {
                    side: OrderSide::Sell,
                    quantity: whole,
                    price: UsOrderPrice::try_new(price("12.34", Currency::Usd)).expect("가격"),
                },
                market: Market::Us,
                side: OrderSide::Sell,
                quantity: SummaryQuantity::Specified(whole.quantity()),
                limit: SummaryLimitPrice::Specified(price("12.34", Currency::Usd)),
                money: money("37.02", Currency::Usd),
            },
            SummaryCase {
                terms: OrderTerms::UsMarketBuy { quantity: whole },
                market: Market::Us,
                side: OrderSide::Buy,
                quantity: SummaryQuantity::Specified(whole.quantity()),
                limit: SummaryLimitPrice::Absent,
                money: money("370.368", Currency::Usd),
            },
            SummaryCase {
                terms: OrderTerms::UsMarketSell {
                    quantity: fractional,
                },
                market: Market::Us,
                side: OrderSide::Sell,
                quantity: SummaryQuantity::Specified(fractional.quantity()),
                limit: SummaryLimitPrice::Absent,
                money: money("15.432", Currency::Usd),
            },
            SummaryCase {
                terms: OrderTerms::UsMarketAmountBuy {
                    amount: UsdOrderAmount::try_new(money("50000", Currency::Usd)).expect("금액"),
                },
                market: Market::Us,
                side: OrderSide::Buy,
                quantity: SummaryQuantity::Absent,
                limit: SummaryLimitPrice::Absent,
                money: money("50000", Currency::Usd),
            },
        ]
    }
    #[test]
    fn all_six_variants_match_independent_order_fields() {
        for case in kr_field_cases().into_iter().chain(us_field_cases()) {
            let token = token(case.terms, "123.456");
            let result = summary(&token, OrderFxRates::missing()).expect("요약");
            assert_eq!(result.market(), case.market);
            assert_eq!(result.side(), case.side);
            assert_eq!(result.quantity(), case.quantity);
            assert_eq!(result.limit_price(), case.limit);
            assert_eq!(result.expected_amount().money(), case.money);
            assert_eq!(
                result.expected_amount().money().currency(),
                case.money.currency()
            );
        }
    }
    #[test]
    fn implausible_rates_and_large_spreads_require_confirmation() {
        for (buy, mid, expected) in [
            ("1", "1", LargeOrderConfirmation::Required),
            ("499.9999", "500", LargeOrderConfirmation::Required),
            ("500", "499.9999", LargeOrderConfirmation::Required),
            ("5000.0001", "5000", LargeOrderConfirmation::Required),
            ("5000", "5000.0001", LargeOrderConfirmation::Required),
            ("500", "500", LargeOrderConfirmation::NotRequired),
            ("5000", "5000", LargeOrderConfirmation::NotRequired),
            ("600", "500", LargeOrderConfirmation::NotRequired),
            ("500", "600", LargeOrderConfirmation::NotRequired),
            ("600.0001", "500", LargeOrderConfirmation::Required),
            ("500", "600.0001", LargeOrderConfirmation::Required),
            (
                "500.00000000000000000000000001",
                "500.00000000000000000000000001",
                LargeOrderConfirmation::Required,
            ),
        ] {
            let token = token_amount("1");
            let result = summary(
                &token,
                OrderFxRates::from_broker_fx_result(
                    rate(buy, FxRateKind::Buy),
                    rate(mid, FxRateKind::Mid),
                ),
            )
            .expect("요약");
            assert_eq!(
                result.expected_amount().confirmation(),
                expected,
                "{buy}, {mid}"
            );
        }
        let large = token_amount("1000000");
        assert_eq!(
            summary(
                &large,
                OrderFxRates::from_broker_fx_result(
                    rate("1", FxRateKind::Buy),
                    rate("1", FxRateKind::Mid)
                )
            )
            .expect("요약")
            .expected_amount()
            .confirmation(),
            LargeOrderConfirmation::Required
        );
    }
    #[test]
    fn display_names_reject_invisible_and_directional_characters() {
        for code in [
            0x00ad, 0x0600, 0x0605, 0x061c, 0x06dd, 0x070f, 0x0890, 0x0891, 0x08e2, 0x180e, 0x200b,
            0x200f, 0x202a, 0x202e, 0x2060, 0x2064, 0x2066, 0x206f, 0xfeff, 0xfff9, 0xfffb,
            0x110bd, 0x110cd, 0x13430, 0x1343f, 0x1bca0, 0x1bca3, 0x1d173, 0x1d17a, 0xe0001,
            0xe0020, 0xe007f, 0x034f, 0x115f, 0x1160, 0x17b4, 0x17b5, 0x180b, 0x180f, 0x2028,
            0x2029, 0x2065, 0x3164, 0xfe00, 0xfe0f, 0xffa0, 0xe0100, 0xe01ef,
        ] {
            let value = format!("이름{}끝", char::from_u32(code).expect("코드 포인트"));
            assert_eq!(
                DisplayInstrumentName::try_new(&value),
                Err(DisplayInstrumentNameError::InvalidName),
                "U+{code:04X}"
            );
        }
    }
    fn pending(token: ExecutionToken) -> PendingOrderConfirmation {
        let name = InstrumentNameFact::from_master_lookup_result(
            token.intent().instrument().clone(),
            test_name(),
        );
        let row = AccountLookupDisplayRow::from_broker_account_result(
            token.intent().account_id().clone(),
            test_suffix(),
        );
        PendingOrderConfirmation::from_execution(
            token,
            name,
            AccountDisplayFact::from_account_lookup_result(row),
            EstimatedOrderFee::Unknown,
            OrderFxRates::missing(),
        )
        .expect("확인 대기 주문")
    }
    #[test]
    fn pending_confirmation_keeps_the_same_order_and_summary_through_recheck() {
        let token = field_test_token();
        let expected_id = token.intent().client_order_id().clone();
        let observation = crate::PriceObservation::from_quote_parts(
            token.intent().instrument().clone(),
            price("100", Currency::Krw),
            at(2),
        )
        .expect("관측");
        let ready = pending(token);
        assert_eq!(
            ready.summary().expected_amount().money(),
            money("100", Currency::Krw)
        );
        let checked = ready.recheck(at(2), observation).expect("재확인");
        assert_eq!(
            checked.summary().expected_amount().money(),
            money("100", Currency::Krw)
        );
        let (original, summary) = checked.into_execution_parts();
        assert_eq!(original.intent().client_order_id(), &expected_id);
        assert_eq!(original.checked_at(), at(2));
        assert_eq!(summary.client_order_prefix().as_str(), "abcdefgh");
    }
    #[test]
    fn invalid_pending_recheck_produces_no_token_or_summary() {
        let token = field_test_token();
        let observation = crate::PriceObservation::from_quote_parts(
            token.intent().instrument().clone(),
            price("100", Currency::Krw),
            at(0),
        )
        .expect("관측");
        assert_eq!(
            pending(token)
                .recheck(at(0), observation)
                .expect_err("시각 역행"),
            crate::ProposalExecutionError::ClockRollback
        );
    }
    #[test]
    fn lookup_row_debug_keeps_only_masked_account_number() {
        let row = AccountLookupDisplayRow::from_broker_account_result(
            AccountId::new(987_654_321),
            test_suffix(),
        );
        let debug = format!("{row:?}");
        assert!(debug.contains("1234"));
        assert!(!debug.contains("987654321"));
        assert!(!debug.contains("123456781234"));
    }
    #[test]
    fn pending_confirmation_rejects_mismatched_lookup_row() {
        let token = field_test_token();
        let name = InstrumentNameFact::from_master_lookup_result(
            token.intent().instrument().clone(),
            test_name(),
        );
        let row =
            AccountLookupDisplayRow::from_broker_account_result(AccountId::new(1), test_suffix());
        let error = PendingOrderConfirmation::from_execution(
            token,
            name,
            AccountDisplayFact::from_account_lookup_result(row),
            EstimatedOrderFee::Unknown,
            OrderFxRates::missing(),
        )
        .expect_err("다른 계좌");
        assert_eq!(error, OrderSummaryError::AccountMismatch);
    }
}
