use crate::{
    Candle, CandlePeriod, CandleStatus, CorporateDelivery, DeliveryStatus, Money, MoneyError,
    PriceAdjustment, Transaction, TransactionKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferClassificationKind {
    InternalCorporateAction,
    ExternalInflow,
    ExternalOutflow,
    Unclassified,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferClassificationError {
    NotTransfer,
    InvalidDelivery,
    InvalidValuation,
    MissingQuantity,
    Arithmetic(MoneyError),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransferOccurrence(usize);
impl TransferOccurrence {
    #[must_use]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }
    #[must_use]
    pub const fn as_usize(&self) -> &usize {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferClassification {
    row: Transaction,
    occurrence: TransferOccurrence,
    effect: TransferEffect,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransferEffect {
    Internal,
    External(crate::FlowDirection, Money),
    Unclassified,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserTransferConfirmation {
    row: Transaction,
    occurrence: TransferOccurrence,
    valuation: Option<Candle>,
}
impl UserTransferConfirmation {
    pub fn into_classification(
        self,
    ) -> Result<TransferClassification, TransferClassificationError> {
        TransferClassification::from_user_confirmation(self)
    }
}
impl TransferClassification {
    pub fn from_corporate_delivery(
        delivery: &CorporateDelivery,
    ) -> Result<Self, TransferClassificationError> {
        if delivery.status() != DeliveryStatus::Received {
            return Err(TransferClassificationError::InvalidDelivery);
        }
        let row = delivery
            .receipt()
            .ok_or(TransferClassificationError::InvalidDelivery)?;
        if row.data().kind != TransactionKind::SecuritiesTransferIn {
            return Err(TransferClassificationError::NotTransfer);
        }
        Ok(Self {
            row: row.clone(),
            occurrence: TransferOccurrence::new(0),
            effect: TransferEffect::Internal,
        })
    }
    pub fn unclassified(
        row: Transaction,
        occurrence: TransferOccurrence,
    ) -> Result<Self, TransferClassificationError> {
        transfer_kind(&row)?;
        Ok(Self {
            row,
            occurrence,
            effect: TransferEffect::Unclassified,
        })
    }
    pub(crate) fn from_user_confirmation(
        confirmation: UserTransferConfirmation,
    ) -> Result<Self, TransferClassificationError> {
        let direction = transfer_kind(&confirmation.row)?;
        let Some(candle) = confirmation.valuation else {
            return Ok(Self {
                row: confirmation.row,
                occurrence: confirmation.occurrence,
                effect: TransferEffect::Unclassified,
            });
        };
        validate_valuation(&confirmation.row, &candle)?;
        let quantity = confirmation
            .row
            .data()
            .quantity
            .ok_or(TransferClassificationError::MissingQuantity)?;
        let value = candle
            .values()
            .close()
            .checked_mul(&quantity)
            .map_err(TransferClassificationError::Arithmetic)?;
        Ok(Self {
            row: confirmation.row,
            occurrence: confirmation.occurrence,
            effect: TransferEffect::External(direction, value),
        })
    }
    #[must_use]
    pub const fn kind(&self) -> TransferClassificationKind {
        match self.effect {
            TransferEffect::Internal => TransferClassificationKind::InternalCorporateAction,
            TransferEffect::Unclassified => TransferClassificationKind::Unclassified,
            TransferEffect::External(crate::FlowDirection::Inflow, _) => {
                TransferClassificationKind::ExternalInflow
            }
            TransferEffect::External(crate::FlowDirection::Outflow, _) => {
                TransferClassificationKind::ExternalOutflow
            }
        }
    }
    #[must_use]
    pub const fn transaction(&self) -> &Transaction {
        &self.row
    }
    #[must_use]
    pub const fn occurrence(&self) -> TransferOccurrence {
        self.occurrence
    }
    pub(crate) const fn effect(&self) -> TransferEffect {
        self.effect
    }
}
fn transfer_kind(row: &Transaction) -> Result<crate::FlowDirection, TransferClassificationError> {
    match row.data().kind {
        TransactionKind::SecuritiesTransferIn => Ok(crate::FlowDirection::Inflow),
        TransactionKind::SecuritiesTransferOut => Ok(crate::FlowDirection::Outflow),
        TransactionKind::Buy
        | TransactionKind::Sell
        | TransactionKind::Deposit
        | TransactionKind::Withdrawal
        | TransactionKind::ExchangeIn
        | TransactionKind::ExchangeOut
        | TransactionKind::Dividend
        | TransactionKind::Interest
        | TransactionKind::Fee
        | TransactionKind::Unknown(_) => Err(TransferClassificationError::NotTransfer),
    }
}
fn validate_valuation(
    row: &Transaction,
    candle: &Candle,
) -> Result<(), TransferClassificationError> {
    if row.data().asset_class != Some(crate::AssetClass::Stock)
        || candle.time().date() != row.data().settlement_date
        || candle.time().period() != CandlePeriod::Daily
        || candle.status() != CandleStatus::Finalized
        || candle.adjustment() != PriceAdjustment::Raw
        || candle.values().currency() != row.data().currency
    {
        return Err(TransferClassificationError::InvalidValuation);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        TransferClassification, TransferClassificationError, TransferClassificationKind,
        TransferEffect, TransferOccurrence, UserTransferConfirmation,
    };
    use crate::cash_ledger::tests::{at, date, money, row, set};
    use crate::{
        AssetClass, CandleObservation, CandleTime, CorporateAction, CorporateActionId,
        CorporateActionKind, CostBasisMethod, Currency, DividendBasis, InstrumentId, LotAssumption,
        LotBook, LotIdentity, Market, NumericInput, Ohlcv, Price, Quantity, ShareCount,
        ShareQuantity, ShareRatio, Symbol,
    };
    use crate::{
        Candle, CandlePeriod, CandleStatus, CorporateDelivery, DeliveryStatus, MoneyError,
        PriceAdjustment, Transaction,
    };
    fn quantity(text: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(text).expect("입력")).expect("수량")
    }
    fn receipt(code: &str) -> Transaction {
        let mut data = row(code, "0", Currency::Usd).data().clone();
        data.asset_class = Some(AssetClass::Stock);
        data.market = Market::Us;
        data.symbol = Some(Symbol::try_new("AAPL").expect("종목"));
        data.quantity = Some(quantity("2"));
        Transaction::try_new(data).expect("입출고")
    }
    fn candle() -> Candle {
        let price = Price::try_new(money("5", Currency::Usd)).expect("종가");
        Candle::try_new(
            InstrumentId::try_new("instrument").expect("ID"),
            CandleTime::try_new(at(86_400), CandlePeriod::Daily).expect("시각"),
            PriceAdjustment::Raw,
            Ohlcv::try_new(price, price, price, price, quantity("1")).expect("봉"),
            CandleObservation::new(at(900_000), CandleStatus::Finalized),
        )
        .expect("일봉")
    }
    fn identity() -> LotIdentity {
        LotIdentity::new(
            InstrumentId::try_new("instrument").expect("ID"),
            Symbol::try_new("AAPL").expect("종목"),
            Market::Us,
            Currency::Usd,
        )
    }
    #[test]
    fn user_confirmation_values_transfer_at_raw_daily_close_and_keeps_unknown_amount_unclassified()
    {
        for (code, kind) in [
            (
                "SECURITIES_TRANSFER_IN",
                TransferClassificationKind::ExternalInflow,
            ),
            (
                "SECURITIES_TRANSFER_OUT",
                TransferClassificationKind::ExternalOutflow,
            ),
        ] {
            let transaction = receipt(code);
            let result = UserTransferConfirmation {
                row: transaction.clone(),
                occurrence: TransferOccurrence::new(0),
                valuation: Some(candle()),
            }
            .into_classification()
            .expect("확인");
            assert_eq!(result.kind(), kind);
            assert_eq!(result.transaction(), &transaction);
            assert_eq!(
                result.effect(),
                TransferEffect::External(
                    if kind == TransferClassificationKind::ExternalInflow {
                        crate::FlowDirection::Inflow
                    } else {
                        crate::FlowDirection::Outflow
                    },
                    money("10", Currency::Usd)
                )
            );
            let rows = set(vec![transaction.clone()]);
            let calendar = crate::external_flow::tests::calendar(
                crate::cash_ledger::tests::ledger(&rows, Currency::Usd, "100")
                    .external_flows(&[result]),
            )
            .expect("달력");
            let flows = crate::external_flow::tests::period_flows(
                calendar.period(crate::cash_ledger::tests::window()),
            )
            .expect("기간");
            assert_eq!(
                flows.flows().first().expect("흐름").amount(),
                money("10", Currency::Usd)
            );
            let unknown = UserTransferConfirmation {
                row: transaction,
                occurrence: TransferOccurrence::new(0),
                valuation: None,
            }
            .into_classification()
            .expect("미평가");
            assert_eq!(unknown.kind(), TransferClassificationKind::Unclassified);
            assert_eq!(unknown.effect(), TransferEffect::Unclassified);
        }
    }
    #[test]
    fn non_transfer_rows_cannot_be_user_classified() {
        assert_eq!(
            TransferClassification::unclassified(
                row("BUY", "1", Currency::Usd),
                TransferOccurrence::new(0)
            ),
            Err(TransferClassificationError::NotTransfer)
        );
        assert_eq!(
            UserTransferConfirmation {
                row: row("BUY", "1", Currency::Usd),
                occurrence: TransferOccurrence::new(0),
                valuation: None
            }
            .into_classification(),
            Err(TransferClassificationError::NotTransfer)
        );
    }
    #[test]
    fn d28_receipt_match_is_the_internal_classification_source() {
        let mut buy = row("BUY", "10", Currency::Usd).data().clone();
        buy.asset_class = Some(AssetClass::Stock);
        buy.market = Market::Us;
        buy.symbol = Some(Symbol::try_new("AAPL").expect("종목"));
        buy.quantity = Some(quantity("10"));
        let mut received = receipt("SECURITIES_TRANSFER_IN").data().clone();
        received.settlement_date = date(4);
        received.transaction_time = Some(at(259_210));
        let transaction = Transaction::try_new(received).expect("입고");
        let rows = set(vec![
            Transaction::try_new(buy).expect("매수"),
            transaction.clone(),
        ]);
        let action = CorporateAction::try_new(
            CorporateActionId::new(InstrumentId::try_new("dividend").expect("ID")),
            date(3),
            CorporateActionKind::StockDividend {
                ratio: ShareRatio::new(
                    ShareCount::try_new(5).expect("구주"),
                    ShareCount::try_new(1).expect("신주"),
                ),
                receipt: Some(transaction.clone()),
                basis: DividendBasis::Zero,
            },
        )
        .expect("행사");
        let book =
            LotBook::from_settled(&rows, identity(), CostBasisMethod::Fifo, &[action], date(5))
                .expect("로트");
        assert!(book.assumptions().contains(&LotAssumption::ReceiptMatch));
        let delivery = book.deliveries().first().expect("입고 연결");
        let classification =
            TransferClassification::from_corporate_delivery(delivery).expect("내부 행사");
        assert_eq!(
            classification.kind(),
            TransferClassificationKind::InternalCorporateAction
        );
        assert_eq!(classification.transaction(), &transaction);
        assert_eq!(classification.effect(), TransferEffect::Internal);
        let calendar = crate::external_flow::tests::calendar(
            crate::cash_ledger::tests::ledger(&rows, Currency::Usd, "90")
                .external_flows(&[classification]),
        )
        .expect("달력");
        assert_eq!(
            crate::external_flow::tests::period_flows(
                calendar.period(crate::cash_ledger::tests::window())
            )
            .expect("기간")
            .flows(),
            &[]
        );
    }
    #[test]
    fn unreceived_unlinked_and_non_receipt_deliveries_are_rejected() {
        let id = CorporateActionId::new(InstrumentId::try_new("action").expect("ID"));
        for status in [DeliveryStatus::Pending, DeliveryStatus::NotReported] {
            let delivery = CorporateDelivery::new(
                id.clone(),
                ShareQuantity::zero(),
                Some(receipt("SECURITIES_TRANSFER_IN")),
                status,
            );
            assert_eq!(
                TransferClassification::from_corporate_delivery(&delivery),
                Err(TransferClassificationError::InvalidDelivery)
            );
        }
        let unlinked = CorporateDelivery::new(
            id.clone(),
            ShareQuantity::zero(),
            None,
            DeliveryStatus::Received,
        );
        assert_eq!(
            TransferClassification::from_corporate_delivery(&unlinked),
            Err(TransferClassificationError::InvalidDelivery)
        );
        let invalid = CorporateDelivery::new(
            id,
            ShareQuantity::zero(),
            Some(row("SELL", "1", Currency::Usd)),
            DeliveryStatus::Received,
        );
        assert_eq!(
            TransferClassification::from_corporate_delivery(&invalid),
            Err(TransferClassificationError::NotTransfer)
        );
    }
    #[test]
    fn valuation_requires_same_date_currency_raw_final_daily_stock_and_quantity() {
        let original = receipt("SECURITIES_TRANSFER_IN");
        let normal = candle();
        let altered = |period, adjustment, status| {
            Candle::try_new(
                normal.instrument().clone(),
                CandleTime::try_new(at(86_400), period).expect("시각"),
                adjustment,
                normal.values(),
                CandleObservation::new(at(900_000), status),
            )
            .expect("봉")
        };
        for value in [
            altered(
                CandlePeriod::Minute,
                PriceAdjustment::Raw,
                CandleStatus::Finalized,
            ),
            altered(
                CandlePeriod::Daily,
                PriceAdjustment::Adjusted(crate::AdjustmentBasis::new(at(900_000))),
                CandleStatus::Finalized,
            ),
            altered(
                CandlePeriod::Daily,
                PriceAdjustment::Raw,
                CandleStatus::InProgress,
            ),
        ] {
            assert_eq!(
                UserTransferConfirmation {
                    row: original.clone(),
                    occurrence: TransferOccurrence::new(0),
                    valuation: Some(value)
                }
                .into_classification(),
                Err(TransferClassificationError::InvalidValuation)
            );
        }
    }
    #[test]
    fn valuation_rejects_mismatched_row_and_missing_or_unrepresentable_quantity() {
        let original = receipt("SECURITIES_TRANSFER_IN");
        let normal = candle();
        let mut wrong_date = original.data().clone();
        wrong_date.settlement_date = date(3);
        let mut wrong_asset = original.data().clone();
        wrong_asset.asset_class = Some(AssetClass::Option);
        let mut wrong_currency = row("SECURITIES_TRANSFER_IN", "0", Currency::Krw)
            .data()
            .clone();
        wrong_currency.asset_class = Some(AssetClass::Stock);
        wrong_currency.quantity = Some(quantity("2"));
        for data in [wrong_date, wrong_asset, wrong_currency] {
            assert_eq!(
                UserTransferConfirmation {
                    row: Transaction::try_new(data).expect("행"),
                    occurrence: TransferOccurrence::new(0),
                    valuation: Some(normal.clone())
                }
                .into_classification(),
                Err(TransferClassificationError::InvalidValuation)
            );
        }
        let mut absent = original.data().clone();
        absent.quantity = None;
        assert_eq!(
            UserTransferConfirmation {
                row: Transaction::try_new(absent).expect("행"),
                occurrence: TransferOccurrence::new(0),
                valuation: Some(normal.clone())
            }
            .into_classification(),
            Err(TransferClassificationError::MissingQuantity)
        );
        let mut huge = original.data().clone();
        huge.quantity = Some(quantity("79228162514264337593543950335"));
        assert_eq!(
            UserTransferConfirmation {
                row: Transaction::try_new(huge).expect("행"),
                occurrence: TransferOccurrence::new(0),
                valuation: Some(normal)
            }
            .into_classification(),
            Err(TransferClassificationError::Arithmetic(
                MoneyError::Overflow
            ))
        );
    }
    #[test]
    fn classification_is_bound_to_each_multiset_occurrence() {
        let transaction = receipt("SECURITIES_TRANSFER_IN");
        let rows = set(vec![transaction.clone(), transaction.clone()]);
        let first = UserTransferConfirmation {
            row: transaction.clone(),
            occurrence: TransferOccurrence::new(0),
            valuation: Some(candle()),
        }
        .into_classification()
        .expect("첫 행");
        let second = UserTransferConfirmation {
            row: transaction.clone(),
            occurrence: TransferOccurrence::new(1),
            valuation: Some(candle()),
        }
        .into_classification()
        .expect("다음 행");
        assert_eq!(second.occurrence().as_usize(), &1);
        let result = crate::external_flow::tests::calendar(
            crate::cash_ledger::tests::ledger(&rows, Currency::Usd, "100")
                .external_flows(std::slice::from_ref(&first)),
        )
        .expect("달력");
        assert_eq!(
            crate::external_flow::tests::period_flows(
                result.period(crate::cash_ledger::tests::window())
            ),
            None
        );
        let confirmed = crate::external_flow::tests::calendar(
            crate::cash_ledger::tests::ledger(&rows, Currency::Usd, "100")
                .external_flows(&[first, second]),
        )
        .expect("달력");
        assert_eq!(
            crate::external_flow::tests::period_flows(
                confirmed.period(crate::cash_ledger::tests::window())
            )
            .expect("기간")
            .flows()
            .len(),
            2
        );
        let unknown = UserTransferConfirmation {
            row: transaction,
            occurrence: TransferOccurrence::new(1),
            valuation: None,
        }
        .into_classification()
        .expect("미분류");
        assert_eq!(unknown.occurrence().as_usize(), &1);
    }
}
