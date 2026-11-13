use crate::{
    CalendarDate, Currency, Market, Money, Quantity, Ratio, Symbol, Timestamp, TransactionKind,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct IsinCode(String);
impl IsinCode {
    pub fn try_new(value: &str) -> Result<Self, TransactionError> {
        if value.is_empty()
            || value.len() > 64
            || !value.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(TransactionError::InvalidIsin);
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssetClassCode(String);
impl AssetClassCode {
    pub fn try_new(value: &str) -> Result<Self, TransactionError> {
        if value.is_empty()
            || value.len() > 64
            || !value.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(TransactionError::InvalidAssetClass);
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnknownAssetClassCode(AssetClassCode);
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssetClass {
    Stock,
    Bond,
    Option,
    Unknown(UnknownAssetClassCode),
}
impl AssetClass {
    #[must_use]
    pub fn new(code: AssetClassCode) -> Self {
        match code.as_str() {
            "STOCK" => Self::Stock,
            "BOND" => Self::Bond,
            "OPTION" => Self::Option,
            _ => Self::Unknown(UnknownAssetClassCode(code)),
        }
    }
    #[must_use]
    pub fn as_code(&self) -> AssetClassCode {
        AssetClassCode(match self {
            Self::Stock => String::from("STOCK"),
            Self::Bond => String::from("BOND"),
            Self::Option => String::from("OPTION"),
            Self::Unknown(code) => return code.0.clone(),
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Cancellation {
    Regular,
    Cancelled,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Settlement {
    Pending,
    Settled,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalculationEffect {
    NoEffect,
    SettledTransaction,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionAnomaly {
    SettledWithoutTime,
    PendingWithTime,
    NonDomesticCancellation,
    PendingExchangeRate,
    PendingNonTrade,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransactionPrice(Money);
impl TransactionPrice {
    pub fn try_new(money: Money) -> Result<Self, TransactionError> {
        if money.amount().as_decimal().is_sign_negative() && !money.amount().as_decimal().is_zero()
        {
            return Err(TransactionError::NegativeAmount);
        }
        Ok(Self(money))
    }
    #[must_use]
    pub const fn as_money(self) -> Money {
        self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionData {
    pub kind: TransactionKind,
    pub market: Market,
    pub currency: Currency,
    pub symbol: Option<Symbol>,
    pub asset_class: Option<AssetClass>,
    pub isin: Option<IsinCode>,
    pub quantity: Option<Quantity>,
    pub filled_price: Option<TransactionPrice>,
    pub amount: Option<Money>,
    pub commission: Money,
    pub tax: Money,
    pub net_amount: Option<Money>,
    pub exchange_rate: Option<Ratio>,
    pub settlement_date: CalendarDate,
    pub settlement: Settlement,
    pub transaction_time: Option<Timestamp>,
    pub cancellation: Cancellation,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction(TransactionData);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionError {
    InvalidIsin,
    InvalidAssetClass,
    CurrencyMismatch,
    NegativeAmount,
}
impl Transaction {
    pub fn try_new(data: TransactionData) -> Result<Self, TransactionError> {
        for money in [
            data.amount,
            Some(data.commission),
            Some(data.tax),
            data.net_amount,
            data.filled_price.map(TransactionPrice::as_money),
        ]
        .into_iter()
        .flatten()
        {
            if money.currency() != data.currency {
                return Err(TransactionError::CurrencyMismatch);
            }
            if money.amount().as_decimal().is_sign_negative()
                && !money.amount().as_decimal().is_zero()
            {
                return Err(TransactionError::NegativeAmount);
            }
        }
        Ok(Self(data))
    }
    #[must_use]
    pub const fn data(&self) -> &TransactionData {
        &self.0
    }
    #[must_use]
    pub fn calculation_effect(&self) -> CalculationEffect {
        if self.0.cancellation == Cancellation::Cancelled
            || self.0.settlement == Settlement::Pending
        {
            CalculationEffect::NoEffect
        } else {
            CalculationEffect::SettledTransaction
        }
    }
    #[must_use]
    pub fn anomalies(&self) -> Vec<TransactionAnomaly> {
        let mut observations = Vec::new();
        if self.0.settlement == Settlement::Settled && self.0.transaction_time.is_none() {
            observations.push(TransactionAnomaly::SettledWithoutTime);
        }
        if self.0.settlement == Settlement::Pending {
            if self.0.transaction_time.is_some() {
                observations.push(TransactionAnomaly::PendingWithTime);
            }
            if self.0.exchange_rate.is_some() {
                observations.push(TransactionAnomaly::PendingExchangeRate);
            }
            match self.0.kind {
                TransactionKind::Buy | TransactionKind::Sell | TransactionKind::Unknown(_) => (),
                TransactionKind::SecuritiesTransferIn
                | TransactionKind::SecuritiesTransferOut
                | TransactionKind::Deposit
                | TransactionKind::Withdrawal
                | TransactionKind::ExchangeIn
                | TransactionKind::ExchangeOut
                | TransactionKind::Dividend
                | TransactionKind::Interest
                | TransactionKind::Fee => observations.push(TransactionAnomaly::PendingNonTrade),
            }
        }
        if self.0.cancellation == Cancellation::Cancelled && self.0.market != Market::Kr {
            observations.push(TransactionAnomaly::NonDomesticCancellation);
        }
        observations
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Amount, AmountInput, AssetClass, AssetClassCode, CalculationEffect, CalendarDate,
        CalendarYear, Cancellation, Currency, DayNumber, IsinCode, Market, Money, MonthNumber,
        Nanosecond, NumericInput, OffsetSeconds, Quantity, Ratio, Settlement, Symbol, Timestamp,
        Transaction, TransactionAnomaly, TransactionCode, TransactionData, TransactionError,
        TransactionKind, TransactionPrice, UnixSeconds,
    };
    use std::collections::BTreeSet;
    fn money(value: &str, currency: Currency) -> Money {
        Money::new(
            Amount::parse(&AmountInput::new(value)).expect("금액"),
            currency,
        )
    }
    fn time() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(1),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn data() -> TransactionData {
        TransactionData {
            kind: TransactionKind::Buy,
            market: Market::Kr,
            currency: Currency::Krw,
            symbol: None,
            asset_class: None,
            isin: None,
            quantity: None,
            filled_price: None,
            amount: None,
            commission: money("0", Currency::Krw),
            tax: money("0", Currency::Krw),
            net_amount: None,
            exchange_rate: None,
            settlement_date: CalendarDate::from_parts(
                CalendarYear::new(2026),
                MonthNumber::new(11),
                DayNumber::new(1),
            )
            .expect("날짜"),
            settlement: Settlement::Settled,
            transaction_time: Some(time()),
            cancellation: Cancellation::Regular,
        }
    }
    #[test]
    fn zero_execution_price_and_independent_time_and_us_cancellation_are_preserved() {
        let mut observed = data();
        observed.filled_price =
            Some(TransactionPrice::try_new(money("0", Currency::Krw)).expect("0 단가"));
        observed.transaction_time = None;
        let settled = Transaction::try_new(observed.clone()).expect("시각 없는 결제");
        assert_eq!(settled.data(), &observed);
        assert_eq!(
            settled.anomalies(),
            vec![TransactionAnomaly::SettledWithoutTime]
        );
        observed.settlement = Settlement::Pending;
        observed.transaction_time = Some(time());
        let pending = Transaction::try_new(observed.clone()).expect("시각 있는 미결제");
        assert_eq!(
            pending.anomalies(),
            vec![TransactionAnomaly::PendingWithTime]
        );
        assert_eq!(pending.calculation_effect(), CalculationEffect::NoEffect);
        observed.market = Market::Us;
        observed.currency = Currency::Usd;
        observed.commission = money("0", Currency::Usd);
        observed.tax = money("0", Currency::Usd);
        observed.filled_price =
            Some(TransactionPrice::try_new(money("0", Currency::Usd)).expect("단가"));
        observed.cancellation = Cancellation::Cancelled;
        observed.settlement = Settlement::Settled;
        let cancelled = Transaction::try_new(observed.clone()).expect("미국 취소 원문");
        assert_eq!(cancelled.data(), &observed);
        assert_eq!(
            cancelled.anomalies(),
            vec![TransactionAnomaly::NonDomesticCancellation]
        );
        assert_eq!(cancelled.calculation_effect(), CalculationEffect::NoEffect);
        assert_eq!(
            observed.filled_price.expect("단가").as_money(),
            money("0", Currency::Usd)
        );
    }
    #[test]
    fn anomalies_preserve_pending_non_trade_exchange_rate_and_unknown_codes() {
        let mut observed = data();
        observed.settlement = Settlement::Pending;
        observed.transaction_time = None;
        observed.exchange_rate =
            Some(Ratio::parse(&NumericInput::try_new("0").expect("입력")).expect("비율"));
        for kind in [
            TransactionKind::Deposit,
            TransactionKind::Withdrawal,
            TransactionKind::ExchangeIn,
            TransactionKind::ExchangeOut,
            TransactionKind::Dividend,
            TransactionKind::Interest,
            TransactionKind::Fee,
            TransactionKind::SecuritiesTransferIn,
            TransactionKind::SecuritiesTransferOut,
        ] {
            observed.kind = kind;
            assert_eq!(
                Transaction::try_new(observed.clone())
                    .expect("보존")
                    .anomalies(),
                vec![
                    TransactionAnomaly::PendingExchangeRate,
                    TransactionAnomaly::PendingNonTrade
                ]
            );
        }
        for kind in [
            TransactionKind::Buy,
            TransactionKind::Sell,
            TransactionKind::new(TransactionCode::try_new("FUTURE").expect("코드")),
        ] {
            observed.kind = kind;
            assert_eq!(
                Transaction::try_new(observed.clone())
                    .expect("미지 종류 보존")
                    .anomalies(),
                vec![TransactionAnomaly::PendingExchangeRate]
            );
        }
        let normal = Transaction::try_new(data()).expect("정상");
        assert_eq!(normal.anomalies(), []);
        assert_eq!(
            normal.calculation_effect(),
            CalculationEffect::SettledTransaction
        );
    }
    #[test]
    fn codes_are_bounded_and_known_asset_classes_never_become_unknown() {
        for code in ["STOCK", "BOND", "OPTION", "FUTURE", "stock"] {
            let class = AssetClass::new(AssetClassCode::try_new(code).expect("코드"));
            assert_eq!(class.as_code().as_str(), code);
            match class {
                AssetClass::Unknown(_) => assert!(["FUTURE", "stock"].contains(&code)),
                AssetClass::Stock | AssetClass::Bond | AssetClass::Option => {
                    assert!(["STOCK", "BOND", "OPTION"].contains(&code));
                }
            }
        }
        for invalid in ["", " ", "A\n", "한글"] {
            assert_eq!(
                AssetClassCode::try_new(invalid),
                Err(TransactionError::InvalidAssetClass)
            );
            assert_eq!(
                IsinCode::try_new(invalid),
                Err(TransactionError::InvalidIsin)
            );
        }
        let maximum = "A".repeat(64);
        assert_eq!(
            AssetClassCode::try_new(&maximum).expect("상한").as_str(),
            maximum
        );
        assert_eq!(IsinCode::try_new(&maximum).expect("상한").as_str(), maximum);
        assert_eq!(
            AssetClassCode::try_new(&"A".repeat(65)),
            Err(TransactionError::InvalidAssetClass)
        );
        assert_eq!(
            IsinCode::try_new(&"A".repeat(65)),
            Err(TransactionError::InvalidIsin)
        );
    }
    #[test]
    fn amount_currency_and_negative_values_remain_rejected() {
        assert_eq!(
            TransactionPrice::try_new(money("-1", Currency::Krw)),
            Err(TransactionError::NegativeAmount)
        );
        let mut observed = data();
        observed.tax = money("-1", Currency::Krw);
        assert_eq!(
            Transaction::try_new(observed.clone()),
            Err(TransactionError::NegativeAmount)
        );
        observed.tax = money("0", Currency::Usd);
        assert_eq!(
            Transaction::try_new(observed),
            Err(TransactionError::CurrencyMismatch)
        );
        let mut zero = data();
        zero.amount = Some(money("-0", Currency::Krw));
        let _accepted = Transaction::try_new(zero).expect("0 금액");
    }
    fn identity_variants(original: &TransactionData) -> Vec<TransactionData> {
        vec![
            {
                let mut changed = original.clone();
                changed.kind = TransactionKind::Sell;
                changed
            },
            {
                let mut changed = original.clone();
                changed.market = Market::Us;
                changed
            },
            {
                let mut changed = original.clone();
                changed.currency = Currency::Usd;
                changed.commission = money("0", Currency::Usd);
                changed.tax = money("0", Currency::Usd);
                changed
            },
            {
                let mut changed = original.clone();
                changed.asset_class = Some(AssetClass::Stock);
                changed
            },
            {
                let mut changed = original.clone();
                changed.symbol = Some(Symbol::try_new("AAPL").expect("종목"));
                changed
            },
            {
                let mut changed = original.clone();
                changed.isin = Some(IsinCode::try_new("US0378331005").expect("ISIN"));
                changed
            },
            {
                let mut changed = original.clone();
                changed.quantity = Some(
                    Quantity::parse(&NumericInput::try_new("1").expect("입력")).expect("수량"),
                );
                changed
            },
            {
                let mut changed = original.clone();
                changed.filled_price =
                    Some(TransactionPrice::try_new(money("0", Currency::Krw)).expect("단가"));
                changed
            },
        ]
    }
    fn value_variants(original: &TransactionData) -> Vec<TransactionData> {
        vec![
            {
                let mut changed = original.clone();
                changed.amount = Some(money("1", Currency::Krw));
                changed
            },
            {
                let mut changed = original.clone();
                changed.commission = money("1", Currency::Krw);
                changed
            },
            {
                let mut changed = original.clone();
                changed.tax = money("1", Currency::Krw);
                changed
            },
            {
                let mut changed = original.clone();
                changed.net_amount = Some(money("1", Currency::Krw));
                changed
            },
            {
                let mut changed = original.clone();
                changed.exchange_rate =
                    Some(Ratio::parse(&NumericInput::try_new("1").expect("입력")).expect("비율"));
                changed
            },
            {
                let mut changed = original.clone();
                changed.settlement_date = CalendarDate::from_parts(
                    CalendarYear::new(2026),
                    MonthNumber::new(11),
                    DayNumber::new(2),
                )
                .expect("날짜");
                changed
            },
            {
                let mut changed = original.clone();
                changed.settlement = Settlement::Pending;
                changed
            },
            {
                let mut changed = original.clone();
                changed.transaction_time = None;
                changed
            },
            {
                let mut changed = original.clone();
                changed.cancellation = Cancellation::Cancelled;
                changed
            },
        ]
    }
    #[test]
    fn sorted_transaction_keys_preserve_every_schema_field_and_decimal_equivalence() {
        let original = data();
        let variants = identity_variants(&original)
            .into_iter()
            .chain(value_variants(&original));
        let mut keys = variants
            .map(|entry| Transaction::try_new(entry).expect("거래"))
            .collect::<BTreeSet<_>>();
        let _inserted = keys.insert(Transaction::try_new(original.clone()).expect("원본"));
        assert_eq!(keys.len(), 18);
        let mut left = original.clone();
        left.amount = Some(money("1.0", Currency::Krw));
        let mut right = original;
        right.amount = Some(money("1.00", Currency::Krw));
        let pair = [
            Transaction::try_new(left).expect("숫자"),
            Transaction::try_new(right).expect("숫자"),
        ];
        assert_eq!(pair.first(), pair.last());
        let mut price_left = data();
        price_left.filled_price =
            Some(TransactionPrice::try_new(money("1", Currency::Krw)).expect("단가"));
        let mut price_right = data();
        price_right.filled_price =
            Some(TransactionPrice::try_new(money("2", Currency::Krw)).expect("단가"));
        assert_eq!(
            Transaction::try_new(price_left)
                .expect("거래")
                .partial_cmp(&Transaction::try_new(price_right).expect("거래")),
            Some(std::cmp::Ordering::Less)
        );
        assert_eq!(pair.into_iter().collect::<BTreeSet<_>>().len(), 1);
    }
}
