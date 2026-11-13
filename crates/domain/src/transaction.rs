use crate::{
    CalendarDate, Currency, Market, Money, Price, Quantity, Ratio, Symbol, Timestamp,
    TransactionCode, TransactionKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetClass(TransactionCode);
impl AssetClass {
    #[must_use]
    pub const fn new(code: TransactionCode) -> Self {
        Self(code)
    }
    #[must_use]
    pub const fn as_code(&self) -> &TransactionCode {
        &self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cancellation {
    Regular,
    Cancelled,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settlement {
    Pending,
    Settled(Timestamp),
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
    pub filled_price: Option<Price>,
    pub amount: Option<Money>,
    pub commission: Money,
    pub tax: Money,
    pub net_amount: Option<Money>,
    pub exchange_rate: Option<Ratio>,
    pub settlement_date: CalendarDate,
    pub settlement: Settlement,
    pub cancellation: Cancellation,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction(TransactionData);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionError {
    InvalidIsin,
    CurrencyMismatch,
    NegativeAmount,
    InvalidPending,
    InvalidExchangeRate,
    InvalidCancellation,
}
impl Transaction {
    pub fn try_new(data: TransactionData) -> Result<Self, TransactionError> {
        for money in [
            data.amount,
            Some(data.commission),
            Some(data.tax),
            data.net_amount,
            data.filled_price.map(|price| price.as_money()),
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
        if (data.market == Market::Kr && data.currency != Currency::Krw)
            || (data.market == Market::Us && data.currency != Currency::Usd)
        {
            return Err(TransactionError::CurrencyMismatch);
        }
        if data.settlement == Settlement::Pending {
            if data.exchange_rate.is_some() || data.cancellation == Cancellation::Cancelled {
                return Err(TransactionError::InvalidPending);
            }
            match data.kind {
                TransactionKind::Buy | TransactionKind::Sell | TransactionKind::Unknown(_) => (),
                TransactionKind::SecuritiesTransferIn
                | TransactionKind::SecuritiesTransferOut
                | TransactionKind::Deposit
                | TransactionKind::Withdrawal
                | TransactionKind::ExchangeIn
                | TransactionKind::ExchangeOut
                | TransactionKind::Dividend
                | TransactionKind::Interest
                | TransactionKind::Fee => return Err(TransactionError::InvalidPending),
            }
        }
        if data.exchange_rate.is_some_and(|entry| {
            entry.as_decimal().is_sign_negative() || entry.as_decimal().is_zero()
        }) {
            return Err(TransactionError::InvalidExchangeRate);
        }
        if data.cancellation == Cancellation::Cancelled && data.market == Market::Us {
            return Err(TransactionError::InvalidCancellation);
        }
        Ok(Self(data))
    }
    #[must_use]
    pub const fn data(&self) -> &TransactionData {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Amount, AmountInput, AssetClass, CalendarDate, CalendarYear, Cancellation, Currency,
        DayNumber, IsinCode, Market, Money, MonthNumber, Nanosecond, NumericInput, OffsetSeconds,
        Price, Quantity, Ratio, Reconciliation, ReconciliationError, RowChange, SettledRowSet,
        Settlement, SettlementWindow, StoredTransaction, Symbol, Timestamp, Transaction,
        TransactionBatch, TransactionCode, TransactionCodeError, TransactionData, TransactionError,
        TransactionKind, UnixSeconds,
    };
    use proptest::{collection, prelude::ProptestConfig, prop_assert, prop_assert_eq, proptest};

    fn date() -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(11),
            DayNumber::new(14),
        )
        .expect("날짜")
    }
    fn time(value: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(value),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn row(code: &str, settlement: Settlement) -> Transaction {
        let zero = Money::new(
            Amount::parse(&AmountInput::new("0")).expect("금액"),
            Currency::Krw,
        );
        Transaction::try_new(TransactionData {
            kind: TransactionKind::new(TransactionCode::try_new(code).expect("코드")),
            market: Market::Kr,
            currency: Currency::Krw,
            symbol: None,
            asset_class: None,
            isin: None,
            quantity: None,
            filled_price: None,
            amount: Some(zero),
            commission: zero,
            tax: zero,
            net_amount: Some(zero),
            exchange_rate: None,
            settlement_date: date(),
            settlement,
            cancellation: Cancellation::Regular,
        })
        .expect("거래")
    }
    fn batch(rows: Vec<Transaction>) -> TransactionBatch {
        TransactionBatch::try_new(rows).expect("행 목록")
    }
    fn apply(set: &SettledRowSet, rows: Vec<Transaction>, at: i64) -> Reconciliation {
        set.reconcile(
            &batch(rows),
            SettlementWindow::try_new(date(), date()).expect("창"),
            time(at),
        )
        .expect("대조")
    }

    #[test]
    fn duplicate_counts_disappearance_reappearance_and_restore() {
        let value = row("DEPOSIT", Settlement::Settled(time(1)));
        let set = SettledRowSet::new();
        let first = apply(&set, vec![value.clone(); 3], 10);
        let second = apply(first.state(), vec![value.clone(); 2], 20);
        assert_eq!(second.state().rows().len(), 3);
        assert_eq!(
            second
                .state()
                .rows()
                .iter()
                .filter(|entry| entry.disappeared_at() == Some(time(20)))
                .count(),
            1
        );
        let third = apply(second.state(), vec![value; 3], 30);
        assert_eq!(third.state().rows().len(), 4);
        assert_eq!(
            third
                .state()
                .rows()
                .iter()
                .filter(|entry| entry.disappeared_at().is_none())
                .count(),
            3
        );
        let restored = SettledRowSet::try_restore(
            third.state().rows().to_vec(),
            third.state().pending().clone(),
        )
        .expect("복원");
        assert_eq!(&restored, third.state());
        assert_eq!(
            third.state().rows().first().expect("행").first_seen(),
            time(10)
        );
    }

    #[test]
    fn provisional_aggregate_is_replaced_without_guessing_settlement_links() {
        let pending = row("BUY", Settlement::Pending);
        let initial = apply(&SettledRowSet::new(), vec![pending], 10);
        assert_eq!(initial.state().rows(), []);
        let settled = row("BUY", Settlement::Settled(time(1)));
        let result = apply(initial.state(), vec![settled; 2], 20);
        assert_eq!(result.state().rows().len(), 2);
        assert_eq!(result.state().pending().as_slice(), []);
        assert!(result.changes().iter().any(|change| match change {
            RowChange::PendingReplaced { .. } => true,
            RowChange::Added(_) | RowChange::Unchanged(_) | RowChange::Disappeared(_) => false,
        }));
    }

    #[test]
    fn cancellation_preserves_original_and_distinguishes_reverse_transaction() {
        let original = row("DEPOSIT", Settlement::Settled(time(1)));
        let initial = apply(&SettledRowSet::new(), vec![original.clone()], 10);
        let mut data = original.data().clone();
        data.cancellation = Cancellation::Cancelled;
        data.settlement = Settlement::Settled(time(2));
        let cancelled = Transaction::try_new(data).expect("취소");
        let result = apply(initial.state(), vec![cancelled.clone()], 20);
        assert_eq!(result.state().rows().len(), 2);
        assert_eq!(
            result
                .state()
                .rows()
                .first()
                .expect("원 행")
                .disappeared_at(),
            Some(time(20))
        );
        assert_eq!(
            result.state().rows().last().expect("취소 행").transaction(),
            &cancelled
        );
        let reverse = apply(
            initial.state(),
            vec![original, row("WITHDRAWAL", Settlement::Settled(time(2)))],
            20,
        );
        assert!(
            reverse
                .state()
                .rows()
                .iter()
                .all(|entry| entry.disappeared_at().is_none())
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn reconciliation_matches_independent_sorted_counts(old in collection::vec(0_u16..4, 0..20), new in collection::vec(0_u16..4, 0..20)) {
            let codes = ["DEPOSIT", "WITHDRAWAL", "DIVIDEND", "FEE"];
            let make = |values: &[u16]| values.iter().map(|value| row(codes.get(usize::from(*value)).expect("코드"), Settlement::Settled(time(1)))).collect::<Vec<_>>();
            let first = apply(&SettledRowSet::new(), make(&old), 10);
            let result = apply(first.state(), make(&new), 20);
            let mut expected = new.clone(); expected.sort_unstable();
            let mut actual = result.state().rows().iter().filter(|entry| entry.disappeared_at().is_none()).map(|entry| codes.iter().position(|code| *code == entry.transaction().data().kind.as_code().as_str()).expect("코드")).collect::<Vec<_>>(); actual.sort_unstable();
            prop_assert_eq!(actual, expected.iter().map(|value| usize::from(*value)).collect::<Vec<_>>());
            let expected_total = (0_u16..4).map(|value| old.iter().filter(|item| **item == value).count().max(new.iter().filter(|item| **item == value).count())).sum::<usize>();
            prop_assert_eq!(result.state().rows().len(), expected_total);
            prop_assert!(result.state().rows().len() >= first.state().rows().len());
            let repeat = apply(result.state(), make(&new), 30);
            prop_assert_eq!(repeat.state(), result.state());
            prop_assert_eq!(repeat.changes(), repeat.state().rows().iter().cloned().map(RowChange::Unchanged).collect::<Vec<_>>());
            let mut reversed_old = old; reversed_old.reverse();
            let reversed_first = apply(&SettledRowSet::new(), make(&reversed_old), 10);
            let mut reversed = new; reversed.reverse();
            let alternate = apply(reversed_first.state(), make(&reversed), 20);
            for code in codes {
                prop_assert_eq!(alternate.state().rows().iter().filter(|entry| entry.transaction().data().kind.as_code().as_str() == code && entry.disappeared_at().is_none()).count(), result.state().rows().iter().filter(|entry| entry.transaction().data().kind.as_code().as_str() == code && entry.disappeared_at().is_none()).count());
            }
        }
    }

    #[test]
    fn every_duplicate_count_pair_is_exact() {
        for stored_count in 0..=16 {
            for received_count in 0..=16 {
                let value = row("DEPOSIT", Settlement::Settled(time(1)));
                let first = apply(&SettledRowSet::new(), vec![value.clone(); stored_count], 10);
                let result = apply(first.state(), vec![value; received_count], 20);
                assert_eq!(
                    result.state().rows().len(),
                    stored_count.max(received_count)
                );
                assert_eq!(
                    result
                        .state()
                        .rows()
                        .iter()
                        .filter(|entry| entry.disappeared_at().is_none())
                        .count(),
                    received_count
                );
            }
        }
    }

    #[test]
    fn kind_codes_are_sealed_and_all_codes_round_trip() {
        for code in [
            "BUY",
            "SELL",
            "SECURITIES_TRANSFER_IN",
            "SECURITIES_TRANSFER_OUT",
            "DEPOSIT",
            "WITHDRAWAL",
            "EXCHANGE_IN",
            "EXCHANGE_OUT",
            "DIVIDEND",
            "INTEREST",
            "FEE",
            "FUTURE",
            "buy",
        ] {
            let kind = TransactionKind::new(TransactionCode::try_new(code).expect("코드"));
            assert_eq!(kind.as_code().as_str(), code);
            match kind {
                TransactionKind::Unknown(_) => assert!(["FUTURE", "buy"].contains(&code)),
                TransactionKind::Buy
                | TransactionKind::Sell
                | TransactionKind::SecuritiesTransferIn
                | TransactionKind::SecuritiesTransferOut
                | TransactionKind::Deposit
                | TransactionKind::Withdrawal
                | TransactionKind::ExchangeIn
                | TransactionKind::ExchangeOut
                | TransactionKind::Dividend
                | TransactionKind::Interest
                | TransactionKind::Fee => assert!(!["FUTURE", "buy"].contains(&code)),
            }
        }
        for value in ["", " ", "A\n", "한글"] {
            assert_eq!(
                TransactionCode::try_new(value),
                Err(TransactionCodeError::InvalidCode)
            );
            assert_eq!(IsinCode::try_new(value), Err(TransactionError::InvalidIsin));
        }
        let maximum = "A".repeat(64);
        assert_eq!(
            TransactionCode::try_new(&maximum).expect("상한").as_str(),
            maximum
        );
        assert_eq!(IsinCode::try_new(&maximum).expect("상한").as_str(), maximum);
        assert_eq!(
            TransactionCode::try_new(&"A".repeat(65)),
            Err(TransactionCodeError::InvalidCode)
        );
        assert_eq!(
            IsinCode::try_new(&"A".repeat(65)),
            Err(TransactionError::InvalidIsin)
        );
        let class = AssetClass::new(TransactionCode::try_new("OPTION").expect("상품"));
        assert_eq!(class.as_code().as_str(), "OPTION");
    }

    fn money(value: &str, currency: Currency) -> Money {
        Money::new(
            Amount::parse(&AmountInput::new(value)).expect("금액"),
            currency,
        )
    }

    #[test]
    fn transaction_validates_money_pending_and_cancellation_without_losing_fields() {
        let original = row("BUY", Settlement::Pending);
        let mut data = original.data().clone();
        data.symbol = Some(Symbol::try_new("005930").expect("종목"));
        data.asset_class = Some(AssetClass::new(
            TransactionCode::try_new("STOCK").expect("상품"),
        ));
        data.isin = Some(IsinCode::try_new("KR7005930003").expect("식별자"));
        data.quantity = Some(
            Quantity::parse(&NumericInput::try_new("0.123456789").expect("입력")).expect("수량"),
        );
        data.filled_price = Some(Price::try_new(money("100", Currency::Krw)).expect("가격"));
        let full = Transaction::try_new(data.clone()).expect("정보 보존");
        assert_eq!(full.data(), &data);
        data.tax = money("-1", Currency::Krw);
        assert_eq!(
            Transaction::try_new(data.clone()),
            Err(TransactionError::NegativeAmount)
        );
        data.tax = money("0", Currency::Usd);
        assert_eq!(
            Transaction::try_new(data.clone()),
            Err(TransactionError::CurrencyMismatch)
        );
        data.tax = money("0", Currency::Krw);
        data.currency = Currency::Usd;
        assert_eq!(
            Transaction::try_new(data.clone()),
            Err(TransactionError::CurrencyMismatch)
        );
        data = original.data().clone();
        data.market = Market::Us;
        assert_eq!(
            Transaction::try_new(data),
            Err(TransactionError::CurrencyMismatch)
        );
    }

    #[test]
    fn pending_and_exchange_rate_are_validated() {
        let original = row("BUY", Settlement::Pending);
        let mut data = original.data().clone();
        data.cancellation = Cancellation::Cancelled;
        assert_eq!(
            Transaction::try_new(data.clone()),
            Err(TransactionError::InvalidPending)
        );
        data.cancellation = Cancellation::Regular;
        data.exchange_rate =
            Some(Ratio::parse(&NumericInput::try_new("1").expect("입력")).expect("환율"));
        assert_eq!(
            Transaction::try_new(data.clone()),
            Err(TransactionError::InvalidPending)
        );
        data.settlement = Settlement::Settled(time(1));
        let _accepted = Transaction::try_new(data.clone()).expect("양수 환율");
        for invalid in ["0", "-1"] {
            data.exchange_rate =
                Some(Ratio::parse(&NumericInput::try_new(invalid).expect("입력")).expect("비율"));
            assert_eq!(
                Transaction::try_new(data.clone()),
                Err(TransactionError::InvalidExchangeRate)
            );
        }
    }

    #[test]
    fn us_cancellation_and_non_trade_pending_are_rejected() {
        let original = row("BUY", Settlement::Pending);
        let mut us = row("DEPOSIT", Settlement::Settled(time(1))).data().clone();
        us.market = Market::Us;
        us.currency = Currency::Usd;
        us.commission = money("0", Currency::Usd);
        us.tax = money("0", Currency::Usd);
        us.amount = None;
        us.net_amount = None;
        us.cancellation = Cancellation::Cancelled;
        assert_eq!(
            Transaction::try_new(us),
            Err(TransactionError::InvalidCancellation)
        );
        for code in [
            "SECURITIES_TRANSFER_IN",
            "SECURITIES_TRANSFER_OUT",
            "DEPOSIT",
            "WITHDRAWAL",
            "EXCHANGE_IN",
            "EXCHANGE_OUT",
            "DIVIDEND",
            "INTEREST",
            "FEE",
        ] {
            let mut invalid = original.data().clone();
            invalid.kind = TransactionKind::new(TransactionCode::try_new(code).expect("코드"));
            assert_eq!(
                Transaction::try_new(invalid),
                Err(TransactionError::InvalidPending)
            );
        }
        let _accepted =
            Transaction::try_new(row("FUTURE", Settlement::Settled(time(1))).data().clone())
                .expect("미지 유형");
    }

    #[test]
    fn restoration_and_synchronization_bounds_are_atomic() {
        let value = row("BUY", Settlement::Settled(time(1)));
        let pending = row("BUY", Settlement::Pending);
        assert_eq!(
            StoredTransaction::try_restore(pending, time(10), None),
            Err(ReconciliationError::PendingInHistory)
        );
        assert_eq!(
            StoredTransaction::try_restore(value.clone(), time(10), Some(time(9))),
            Err(ReconciliationError::InvalidHistory)
        );
        let saved = StoredTransaction::try_restore(value.clone(), time(10), Some(time(10)))
            .expect("동시각");
        assert_eq!(saved.first_seen(), time(10));
        assert_eq!(saved.disappeared_at(), Some(time(10)));
        assert_eq!(saved.transaction(), &value);
        assert_eq!(SettledRowSet::default(), SettledRowSet::new());
        assert_eq!(
            SettledRowSet::try_restore(vec![], batch(vec![value.clone()])),
            Err(ReconciliationError::InvalidHistory)
        );
        assert_eq!(
            TransactionBatch::try_new(vec![value.clone(); 4097]),
            Err(ReconciliationError::TooManyRows)
        );
        let _maximum = TransactionBatch::try_new(vec![value; 4096]).expect("상한");
        let _maximum_history =
            SettledRowSet::try_restore(vec![saved.clone(); 4096], batch(vec![])).expect("상한");
        assert_eq!(
            SettledRowSet::try_restore(vec![saved; 4097], batch(vec![])),
            Err(ReconciliationError::TooManyRows)
        );
    }

    #[test]
    fn synchronization_time_and_window_are_checked() {
        let value = row("BUY", Settlement::Settled(time(1)));
        let first = apply(&SettledRowSet::new(), vec![value], 10);
        assert_eq!(
            first.state().reconcile(
                &batch(vec![]),
                SettlementWindow::try_new(date(), date()).expect("창"),
                time(9)
            ),
            Err(ReconciliationError::InvalidHistory)
        );
        let missing = apply(first.state(), vec![], 20);
        assert_eq!(
            missing.state().reconcile(
                &batch(vec![]),
                SettlementWindow::try_new(date(), date()).expect("창"),
                time(15)
            ),
            Err(ReconciliationError::InvalidHistory)
        );
    }

    #[test]
    fn synchronization_window_preserves_outside_rows() {
        let value = row("BUY", Settlement::Settled(time(1)));
        let pending = row("BUY", Settlement::Pending);
        let first = apply(&SettledRowSet::new(), vec![value.clone()], 10);
        let yesterday = CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(11),
            DayNumber::new(13),
        )
        .expect("전날");
        let tomorrow = CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(11),
            DayNumber::new(15),
        )
        .expect("다음날");
        assert_eq!(
            SettlementWindow::try_new(date(), yesterday),
            Err(ReconciliationError::InvalidWindow)
        );
        for other in [yesterday, tomorrow] {
            let window = SettlementWindow::try_new(other, other).expect("창");
            assert_eq!(window.from(), other);
            assert_eq!(window.to(), other);
            assert_eq!(
                first
                    .state()
                    .reconcile(&batch(vec![value.clone()]), window, time(20)),
                Err(ReconciliationError::OutsideWindow)
            );
            assert_eq!(
                first
                    .state()
                    .reconcile(&batch(vec![]), window, time(20))
                    .expect("창 밖 보존")
                    .state(),
                first.state()
            );
            let provisional = apply(&SettledRowSet::new(), vec![pending.clone()], 10);
            assert_eq!(
                provisional
                    .state()
                    .reconcile(&batch(vec![]), window, time(20))
                    .expect("임시 창 밖 보존")
                    .state(),
                provisional.state()
            );
        }
        let repeated = apply(
            &apply(&SettledRowSet::new(), vec![pending.clone(); 2], 10)
                .state()
                .clone(),
            vec![pending],
            20,
        );
        assert_eq!(repeated.state().pending().as_slice().len(), 1);
    }
}
