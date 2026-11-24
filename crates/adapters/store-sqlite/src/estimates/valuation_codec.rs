use super::codec::{
    Decoder, Encoder, invalid, put_currency, put_date, put_known_market, put_money, put_price,
    put_quantity, put_ratio, put_timestamp, take_currency, take_date, take_known_market,
    take_money, take_price, take_quantity, take_ratio, take_timestamp,
};
use domain::{
    AdjustmentBasis, AssetClass, AssetClassCode, BuyingPower, Cancellation, Candle,
    CandleObservation, CandlePeriod, CandleStatus, CandleTime, CashAccessDenial, CashMissingData,
    CashObservation, CashObservationFailure, CashReadCapability, CashRequestRejection,
    CashResponseMismatch, CashRetryDelay, CashUnavailable, CashUnavailableCause, CorporateActionId,
    CostBasisMethod, Currency, DeliveryStatus, FlowDirection, FxConversionError, FxPair, FxRate,
    FxRateKind, FxSource, InstrumentId, IntegerText, IsinCode, LotAssumption, LotCost,
    LotCostCurrency, LotError, LotIdentity, LotListing, LotTaxStatus, Market, MarketCode, Money,
    MoneyError, Nanosecond, NumericInput, ObservedCloseEvidence, Ohlcv, PortfolioError,
    PriceAdjustment, QuoteInstrument, SavedConfirmedFlowPeriod, SavedCorporateDelivery,
    SavedCostPool, SavedExternalFlow, SavedFractionalEntitlement, SavedLot, SavedLotBook,
    SavedLotRealization, SavedQuoteInstrument, SavedSnapshotPosition, SavedValuationSnapshot,
    Settlement, SettlementWindow, ShareQuantity, SnapshotCashBasis, SnapshotDates, SnapshotError,
    SnapshotPriceBasis, SnapshotSource, SpanSeconds, Symbol, TimeError, TimeInterval, TimeSpan,
    Transaction, TransactionCode, TransactionData, TransactionKind, TransactionPrice,
    ValuationDayResult, ValuationSnapshot,
};
use std::collections::BTreeSet;

pub(crate) fn put_market(out: &mut Encoder, market: &Market) {
    out.push(market.as_code().as_str());
}
pub(crate) fn take_market(input: &mut Decoder<'_>) -> rusqlite::Result<Market> {
    let code = MarketCode::try_new(input.next()?).map_err(invalid)?;
    Ok(Market::new(code))
}
fn put_money_error(out: &mut Encoder, error: MoneyError) {
    out.push(match error {
        MoneyError::CurrencyMismatch => "currency_mismatch",
        MoneyError::Overflow => "overflow",
        MoneyError::PrecisionLoss => "precision_loss",
        MoneyError::DivisionByZero => "division_by_zero",
    });
}
fn take_money_error(input: &mut Decoder<'_>) -> rusqlite::Result<MoneyError> {
    match input.next()? {
        "currency_mismatch" => Ok(MoneyError::CurrencyMismatch),
        "overflow" => Ok(MoneyError::Overflow),
        "precision_loss" => Ok(MoneyError::PrecisionLoss),
        "division_by_zero" => Ok(MoneyError::DivisionByZero),
        _ => Err(invalid(())),
    }
}
fn put_lot_error(out: &mut Encoder, error: LotError) {
    match error {
        LotError::Arithmetic(inner) => {
            out.push("arithmetic");
            put_money_error(out, inner);
        }
        LotError::InvalidRatio => out.push("invalid_ratio"),
        LotError::InvalidQuantity => out.push("invalid_quantity"),
        LotError::NegativeCost => out.push("negative_cost"),
        LotError::InvalidIdentity => out.push("invalid_identity"),
        LotError::MissingData => out.push("missing_data"),
        LotError::InsufficientQuantity => out.push("insufficient_quantity"),
        LotError::AmbiguousOrder => out.push("ambiguous_order"),
        LotError::DuplicateAction => out.push("duplicate_action"),
        LotError::FutureAction => out.push("future_action"),
        LotError::InvalidReceipt => out.push("invalid_receipt"),
        LotError::CurrencyMismatch => out.push("currency_mismatch"),
        LotError::ManualActionRequired => out.push("manual_action_required"),
        LotError::Delisted => out.push("delisted"),
        LotError::FractionalCashRequired => out.push("fractional_cash_required"),
    }
}
fn take_lot_error(input: &mut Decoder<'_>) -> rusqlite::Result<LotError> {
    match input.next()? {
        "arithmetic" => Ok(LotError::Arithmetic(take_money_error(input)?)),
        "invalid_ratio" => Ok(LotError::InvalidRatio),
        "invalid_quantity" => Ok(LotError::InvalidQuantity),
        "negative_cost" => Ok(LotError::NegativeCost),
        "invalid_identity" => Ok(LotError::InvalidIdentity),
        "missing_data" => Ok(LotError::MissingData),
        "insufficient_quantity" => Ok(LotError::InsufficientQuantity),
        "ambiguous_order" => Ok(LotError::AmbiguousOrder),
        "duplicate_action" => Ok(LotError::DuplicateAction),
        "future_action" => Ok(LotError::FutureAction),
        "invalid_receipt" => Ok(LotError::InvalidReceipt),
        "currency_mismatch" => Ok(LotError::CurrencyMismatch),
        "manual_action_required" => Ok(LotError::ManualActionRequired),
        "delisted" => Ok(LotError::Delisted),
        "fractional_cash_required" => Ok(LotError::FractionalCashRequired),
        _ => Err(invalid(())),
    }
}
fn put_cash_unavailable(out: &mut Encoder, error: CashUnavailable) {
    match error {
        CashUnavailable::UnknownOpening => out.push("unknown_opening"),
        CashUnavailable::IncompleteResponse => out.push("incomplete_response"),
        CashUnavailable::UnknownCurrency => out.push("unknown_currency"),
        CashUnavailable::UnconfirmedCancellation => out.push("unconfirmed_cancellation"),
        CashUnavailable::UnclassifiedRow => out.push("unclassified_row"),
        CashUnavailable::MissingNetAmount => out.push("missing_net_amount"),
        CashUnavailable::CurrencyMismatch => out.push("currency_mismatch"),
        CashUnavailable::MissingReportedCash => out.push("missing_reported_cash"),
        CashUnavailable::InvalidReportedCash => out.push("invalid_reported_cash"),
        CashUnavailable::ReportedDifference => out.push("reported_difference"),
        CashUnavailable::Arithmetic(inner) => {
            out.push("arithmetic");
            put_money_error(out, inner);
        }
        CashUnavailable::TransferCashUnknown => out.push("transfer_cash_unknown"),
        CashUnavailable::StaleReportedCash => out.push("stale_reported_cash"),
        CashUnavailable::MissingSettlementTime => out.push("missing_settlement_time"),
    }
}
fn take_cash_unavailable(input: &mut Decoder<'_>) -> rusqlite::Result<CashUnavailable> {
    match input.next()? {
        "unknown_opening" => Ok(CashUnavailable::UnknownOpening),
        "incomplete_response" => Ok(CashUnavailable::IncompleteResponse),
        "unknown_currency" => Ok(CashUnavailable::UnknownCurrency),
        "unconfirmed_cancellation" => Ok(CashUnavailable::UnconfirmedCancellation),
        "unclassified_row" => Ok(CashUnavailable::UnclassifiedRow),
        "missing_net_amount" => Ok(CashUnavailable::MissingNetAmount),
        "currency_mismatch" => Ok(CashUnavailable::CurrencyMismatch),
        "missing_reported_cash" => Ok(CashUnavailable::MissingReportedCash),
        "invalid_reported_cash" => Ok(CashUnavailable::InvalidReportedCash),
        "reported_difference" => Ok(CashUnavailable::ReportedDifference),
        "arithmetic" => Ok(CashUnavailable::Arithmetic(take_money_error(input)?)),
        "transfer_cash_unknown" => Ok(CashUnavailable::TransferCashUnknown),
        "stale_reported_cash" => Ok(CashUnavailable::StaleReportedCash),
        "missing_settlement_time" => Ok(CashUnavailable::MissingSettlementTime),
        _ => Err(invalid(())),
    }
}
fn put_fx_error(out: &mut Encoder, error: FxConversionError) {
    match error {
        FxConversionError::CurrencyMismatch => out.push("currency_mismatch"),
        FxConversionError::MidRequired => out.push("mid_required"),
        FxConversionError::Arithmetic(inner) => {
            out.push("arithmetic");
            put_money_error(out, inner);
        }
        FxConversionError::TooManyRates => out.push("too_many_rates"),
        FxConversionError::OutsideValidity => out.push("outside_validity"),
        FxConversionError::PairMismatch => out.push("pair_mismatch"),
        FxConversionError::UnsupportedDirection => out.push("unsupported_direction"),
    }
}
fn take_fx_error(input: &mut Decoder<'_>) -> rusqlite::Result<FxConversionError> {
    match input.next()? {
        "currency_mismatch" => Ok(FxConversionError::CurrencyMismatch),
        "mid_required" => Ok(FxConversionError::MidRequired),
        "arithmetic" => Ok(FxConversionError::Arithmetic(take_money_error(input)?)),
        "too_many_rates" => Ok(FxConversionError::TooManyRates),
        "outside_validity" => Ok(FxConversionError::OutsideValidity),
        "pair_mismatch" => Ok(FxConversionError::PairMismatch),
        "unsupported_direction" => Ok(FxConversionError::UnsupportedDirection),
        _ => Err(invalid(())),
    }
}
fn put_time_error(out: &mut Encoder, error: TimeError) {
    out.push(match error {
        TimeError::InvalidComponent => "invalid_component",
        TimeError::OutOfRange => "out_of_range",
        TimeError::Overflow => "overflow",
        TimeError::NegativeSpan => "negative_span",
    });
}
fn take_time_error(input: &mut Decoder<'_>) -> rusqlite::Result<TimeError> {
    match input.next()? {
        "invalid_component" => Ok(TimeError::InvalidComponent),
        "out_of_range" => Ok(TimeError::OutOfRange),
        "overflow" => Ok(TimeError::Overflow),
        "negative_span" => Ok(TimeError::NegativeSpan),
        _ => Err(invalid(())),
    }
}
fn put_snapshot_error(out: &mut Encoder, error: SnapshotError) {
    match error {
        SnapshotError::CalendarMismatch => out.push("calendar_mismatch"),
        SnapshotError::ClosedDay => out.push("closed_day"),
        SnapshotError::MissingPrice => out.push("missing_price"),
        SnapshotError::InvalidCandle => out.push("invalid_candle"),
        SnapshotError::MissingFx => out.push("missing_fx"),
        SnapshotError::InvalidFx => out.push("invalid_fx"),
        SnapshotError::MissingCash => out.push("missing_cash"),
        SnapshotError::IncompleteValuation => out.push("incomplete_valuation"),
        SnapshotError::MissingPrevious => out.push("missing_previous"),
        SnapshotError::WrongPeriod => out.push("wrong_period"),
        SnapshotError::MissingFlows => out.push("missing_flows"),
        SnapshotError::DuplicateInstrument => out.push("duplicate_instrument"),
        SnapshotError::TooManyInputs => out.push("too_many_inputs"),
        SnapshotError::PendingCorporateCash => out.push("pending_corporate_cash"),
        SnapshotError::MissingHistory => out.push("missing_history"),
        SnapshotError::OutsideConfirmedPeriod => out.push("outside_confirmed_period"),
        SnapshotError::WrongCashBasis => out.push("wrong_cash_basis"),
        SnapshotError::MissingExecutionDate => out.push("missing_execution_date"),
        SnapshotError::MissingReportedValue => out.push("missing_reported_value"),
        SnapshotError::MissingFlowTime => out.push("missing_flow_time"),
        SnapshotError::UnsupportedStockTime => out.push("unsupported_stock_time"),
        SnapshotError::Lot(inner) => {
            out.push("lot");
            put_lot_error(out, inner);
        }
        SnapshotError::Cash(inner) => {
            out.push("cash");
            put_cash_unavailable(out, inner);
        }
        SnapshotError::Arithmetic(inner) => {
            out.push("arithmetic");
            put_money_error(out, inner);
        }
        SnapshotError::Conversion(inner) => {
            out.push("conversion");
            put_fx_error(out, inner);
        }
        SnapshotError::Time(inner) => {
            out.push("time");
            put_time_error(out, inner);
        }
    }
}
fn take_snapshot_error(input: &mut Decoder<'_>) -> rusqlite::Result<SnapshotError> {
    match input.next()? {
        "calendar_mismatch" => Ok(SnapshotError::CalendarMismatch),
        "closed_day" => Ok(SnapshotError::ClosedDay),
        "missing_price" => Ok(SnapshotError::MissingPrice),
        "invalid_candle" => Ok(SnapshotError::InvalidCandle),
        "missing_fx" => Ok(SnapshotError::MissingFx),
        "invalid_fx" => Ok(SnapshotError::InvalidFx),
        "missing_cash" => Ok(SnapshotError::MissingCash),
        "incomplete_valuation" => Ok(SnapshotError::IncompleteValuation),
        "missing_previous" => Ok(SnapshotError::MissingPrevious),
        "wrong_period" => Ok(SnapshotError::WrongPeriod),
        "missing_flows" => Ok(SnapshotError::MissingFlows),
        "duplicate_instrument" => Ok(SnapshotError::DuplicateInstrument),
        "too_many_inputs" => Ok(SnapshotError::TooManyInputs),
        "pending_corporate_cash" => Ok(SnapshotError::PendingCorporateCash),
        "missing_history" => Ok(SnapshotError::MissingHistory),
        "outside_confirmed_period" => Ok(SnapshotError::OutsideConfirmedPeriod),
        "wrong_cash_basis" => Ok(SnapshotError::WrongCashBasis),
        "missing_execution_date" => Ok(SnapshotError::MissingExecutionDate),
        "missing_reported_value" => Ok(SnapshotError::MissingReportedValue),
        "missing_flow_time" => Ok(SnapshotError::MissingFlowTime),
        "unsupported_stock_time" => Ok(SnapshotError::UnsupportedStockTime),
        "lot" => Ok(SnapshotError::Lot(take_lot_error(input)?)),
        "cash" => Ok(SnapshotError::Cash(take_cash_unavailable(input)?)),
        "arithmetic" => Ok(SnapshotError::Arithmetic(take_money_error(input)?)),
        "conversion" => Ok(SnapshotError::Conversion(take_fx_error(input)?)),
        "time" => Ok(SnapshotError::Time(take_time_error(input)?)),
        _ => Err(invalid(())),
    }
}
fn put_money_result(out: &mut Encoder, value: Result<Money, SnapshotError>) {
    match value {
        Ok(money) => {
            out.push("ok");
            put_money(out, money);
        }
        Err(error) => {
            out.push("err");
            put_snapshot_error(out, error);
        }
    }
}
fn take_money_result(input: &mut Decoder<'_>) -> rusqlite::Result<Result<Money, SnapshotError>> {
    match input.next()? {
        "ok" => Ok(Ok(take_money(input)?)),
        "err" => Ok(Err(take_snapshot_error(input)?)),
        _ => Err(invalid(())),
    }
}
pub(crate) fn put_fx_rate(out: &mut Encoder, rate: FxRate) {
    out.push(
        &rate
            .quote_per_base()
            .as_money()
            .amount()
            .as_decimal()
            .to_string(),
    );
    out.push(match rate.pair() {
        FxPair::UsdKrw => "USD_KRW",
        FxPair::KrwUsd => "KRW_USD",
    });
    out.push(match rate.kind() {
        FxRateKind::Mid => "mid",
        FxRateKind::Buy => "buy",
    });
    put_timestamp(out, rate.as_of());
    put_timestamp(out, rate.valid_until());
    out.push(match rate.source() {
        FxSource::TossOpenApi => "toss_open_api",
    });
}
pub(crate) fn take_fx_rate(input: &mut Decoder<'_>) -> rusqlite::Result<FxRate> {
    let numeric = NumericInput::try_new(input.next()?).map_err(invalid)?;
    let pair = match input.next()? {
        "USD_KRW" => FxPair::UsdKrw,
        "KRW_USD" => FxPair::KrwUsd,
        _ => return Err(invalid(())),
    };
    let kind = match input.next()? {
        "mid" => FxRateKind::Mid,
        "buy" => FxRateKind::Buy,
        _ => return Err(invalid(())),
    };
    let start = take_timestamp(input)?;
    let end = take_timestamp(input)?;
    let validity = TimeInterval::try_new(start, end).map_err(invalid)?;
    let source = match input.next()? {
        "toss_open_api" => FxSource::TossOpenApi,
        _ => return Err(invalid(())),
    };
    FxRate::parse(&numeric, pair, kind, validity, source).map_err(invalid)
}
pub(crate) fn put_candle(out: &mut Encoder, candle: &Candle) {
    out.push(candle.instrument().as_str());
    put_timestamp(out, candle.time().timestamp());
    out.push(match candle.time().period() {
        CandlePeriod::Daily => "daily",
        CandlePeriod::Minute => "minute",
    });
    match candle.adjustment() {
        PriceAdjustment::Raw => out.push("raw"),
        PriceAdjustment::Adjusted(basis) => {
            out.push("adjusted");
            put_timestamp(out, *basis.as_timestamp());
        }
    }
    put_price(out, candle.values().open());
    put_price(out, candle.values().high());
    put_price(out, candle.values().low());
    put_price(out, candle.values().close());
    put_quantity(out, candle.values().volume());
    put_timestamp(out, candle.collected_at());
    out.push(match candle.status() {
        CandleStatus::Finalized => "finalized",
        CandleStatus::InProgress => "in_progress",
    });
}
pub(crate) fn take_candle(input: &mut Decoder<'_>) -> rusqlite::Result<Candle> {
    let instrument = InstrumentId::try_new(input.next()?).map_err(invalid)?;
    let timestamp = take_timestamp(input)?;
    let period = match input.next()? {
        "daily" => CandlePeriod::Daily,
        "minute" => CandlePeriod::Minute,
        _ => return Err(invalid(())),
    };
    let time = CandleTime::try_new(timestamp, period).map_err(invalid)?;
    let adjustment = match input.next()? {
        "raw" => PriceAdjustment::Raw,
        "adjusted" => PriceAdjustment::Adjusted(AdjustmentBasis::new(take_timestamp(input)?)),
        _ => return Err(invalid(())),
    };
    let open = take_price(input)?;
    let high = take_price(input)?;
    let low = take_price(input)?;
    let close = take_price(input)?;
    let volume = take_quantity(input)?;
    let values = Ohlcv::try_new(open, high, low, close, volume).map_err(invalid)?;
    let collected_at = take_timestamp(input)?;
    let status = match input.next()? {
        "finalized" => CandleStatus::Finalized,
        "in_progress" => CandleStatus::InProgress,
        _ => return Err(invalid(())),
    };
    Candle::try_new(
        instrument,
        time,
        adjustment,
        values,
        CandleObservation::new(collected_at, status),
    )
    .map_err(invalid)
}
fn put_opt_money(out: &mut Encoder, value: Option<Money>) {
    match value {
        Some(money) => {
            out.push("some");
            put_money(out, money);
        }
        None => out.push("none"),
    }
}
fn take_opt_money(input: &mut Decoder<'_>) -> rusqlite::Result<Option<Money>> {
    match input.next()? {
        "some" => Ok(Some(take_money(input)?)),
        "none" => Ok(None),
        _ => Err(invalid(())),
    }
}
fn put_transaction_meta(out: &mut Encoder, data: &TransactionData) {
    out.push(data.kind.as_code().as_str());
    put_market(out, &data.market);
    put_currency(out, data.currency);
    match &data.symbol {
        Some(symbol) => {
            out.push("some");
            out.push(symbol.as_str());
        }
        None => out.push("none"),
    }
    match &data.asset_class {
        Some(asset) => {
            out.push("some");
            out.push(asset.as_code().as_str());
        }
        None => out.push("none"),
    }
    match &data.isin {
        Some(isin) => {
            out.push("some");
            out.push(isin.as_str());
        }
        None => out.push("none"),
    }
    match data.quantity {
        Some(quantity) => {
            out.push("some");
            put_quantity(out, quantity);
        }
        None => out.push("none"),
    }
}
fn put_transaction(out: &mut Encoder, transaction: &Transaction) {
    let data = transaction.data();
    put_transaction_meta(out, data);
    put_opt_money(out, data.filled_price.map(TransactionPrice::as_money));
    put_opt_money(out, data.amount);
    put_money(out, data.commission);
    put_money(out, data.tax);
    put_opt_money(out, data.net_amount);
    match data.exchange_rate {
        Some(rate) => {
            out.push("some");
            put_ratio(out, rate);
        }
        None => out.push("none"),
    }
    put_date(out, data.settlement_date);
    out.push(match data.settlement {
        Settlement::Pending => "pending",
        Settlement::Settled => "settled",
    });
    match data.transaction_time {
        Some(time) => {
            out.push("some");
            put_timestamp(out, time);
        }
        None => out.push("none"),
    }
    out.push(match data.cancellation {
        Cancellation::Regular => "regular",
        Cancellation::Cancelled => "cancelled",
    });
}
struct TxMeta {
    kind: TransactionKind,
    market: Market,
    currency: Currency,
    symbol: Option<Symbol>,
    asset_class: Option<AssetClass>,
    isin: Option<IsinCode>,
    quantity: Option<domain::Quantity>,
}
fn take_transaction_meta(input: &mut Decoder<'_>) -> rusqlite::Result<TxMeta> {
    let kind = TransactionKind::new(TransactionCode::try_new(input.next()?).map_err(invalid)?);
    let market = take_market(input)?;
    let currency = take_currency(input)?;
    let symbol = match input.next()? {
        "some" => Some(Symbol::try_new(input.next()?).map_err(invalid)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    let asset_class = match input.next()? {
        "some" => Some(AssetClass::new(
            AssetClassCode::try_new(input.next()?).map_err(invalid)?,
        )),
        "none" => None,
        _ => return Err(invalid(())),
    };
    let isin = match input.next()? {
        "some" => Some(IsinCode::try_new(input.next()?).map_err(invalid)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    let quantity = match input.next()? {
        "some" => Some(take_quantity(input)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    Ok(TxMeta {
        kind,
        market,
        currency,
        symbol,
        asset_class,
        isin,
        quantity,
    })
}
fn take_transaction(input: &mut Decoder<'_>) -> rusqlite::Result<Transaction> {
    let meta = take_transaction_meta(input)?;
    let filled_price = take_opt_money(input)?
        .map(TransactionPrice::try_new)
        .transpose()
        .map_err(invalid)?;
    let amount = take_opt_money(input)?;
    let commission = take_money(input)?;
    let tax = take_money(input)?;
    let net_amount = take_opt_money(input)?;
    let exchange_rate = match input.next()? {
        "some" => Some(take_ratio(input)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    let settlement_date = take_date(input)?;
    let settlement = match input.next()? {
        "pending" => Settlement::Pending,
        "settled" => Settlement::Settled,
        _ => return Err(invalid(())),
    };
    let transaction_time = match input.next()? {
        "some" => Some(take_timestamp(input)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    let cancellation = match input.next()? {
        "regular" => Cancellation::Regular,
        "cancelled" => Cancellation::Cancelled,
        _ => return Err(invalid(())),
    };
    Transaction::try_new(TransactionData {
        kind: meta.kind,
        market: meta.market,
        currency: meta.currency,
        symbol: meta.symbol,
        asset_class: meta.asset_class,
        isin: meta.isin,
        quantity: meta.quantity,
        filled_price,
        amount,
        commission,
        tax,
        net_amount,
        exchange_rate,
        settlement_date,
        settlement,
        transaction_time,
        cancellation,
    })
    .map_err(invalid)
}
fn put_share_quantity(out: &mut Encoder, quantity: &ShareQuantity) {
    out.push(quantity.numerator().as_str());
    out.push(quantity.denominator().as_str());
}
fn take_share_quantity(input: &mut Decoder<'_>) -> rusqlite::Result<ShareQuantity> {
    let numerator = IntegerText::try_new(input.next()?).map_err(invalid)?;
    let denominator = IntegerText::try_new(input.next()?).map_err(invalid)?;
    ShareQuantity::from_parts(&numerator, &denominator).map_err(invalid)
}
fn put_lot_cost(out: &mut Encoder, cost: LotCost) {
    match cost {
        LotCost::Known(money) => {
            out.push("known");
            put_money(out, money);
        }
        LotCost::Unknown => out.push("unknown"),
    }
}
fn take_lot_cost(input: &mut Decoder<'_>) -> rusqlite::Result<LotCost> {
    match input.next()? {
        "known" => Ok(LotCost::Known(take_money(input)?)),
        "unknown" => Ok(LotCost::Unknown),
        _ => Err(invalid(())),
    }
}
fn put_lot_assumptions(out: &mut Encoder, values: &BTreeSet<LotAssumption>) {
    out.push_usize(values.len());
    for item in values {
        out.push(match item {
            LotAssumption::TransactionOrder => "transaction_order",
            LotAssumption::DelistedSettlement => "delisted_settlement",
            LotAssumption::ReceiptMatch => "receipt_match",
            LotAssumption::FractionalReceiptQuantity => "fractional_receipt_quantity",
        });
    }
}
fn take_lot_assumptions(input: &mut Decoder<'_>) -> rusqlite::Result<BTreeSet<LotAssumption>> {
    let count = input.next_usize()?;
    let mut values = BTreeSet::new();
    for _ in 0..count {
        let item = match input.next()? {
            "transaction_order" => LotAssumption::TransactionOrder,
            "delisted_settlement" => LotAssumption::DelistedSettlement,
            "receipt_match" => LotAssumption::ReceiptMatch,
            "fractional_receipt_quantity" => LotAssumption::FractionalReceiptQuantity,
            _ => return Err(invalid(())),
        };
        values.insert(item);
    }
    Ok(values)
}
fn put_lot_book_corporate(out: &mut Encoder, book: &SavedLotBook) {
    out.push(match book.listing {
        LotListing::Listed => "listed",
        LotListing::Delisted => "delisted",
    });
    out.push_usize(book.applied.len());
    for action in &book.applied {
        out.push(action.instrument_id().as_str());
    }
    out.push_usize(book.fractional.len());
    for item in &book.fractional {
        out.push(item.action.instrument_id().as_str());
        put_share_quantity(out, &item.quantity);
        put_lot_cost(out, item.cost);
    }
    out.push_usize(book.deliveries.len());
    for item in &book.deliveries {
        out.push(item.action.instrument_id().as_str());
        put_share_quantity(out, &item.quantity);
        match &item.receipt {
            Some(receipt) => {
                out.push("some");
                put_transaction(out, receipt);
            }
            None => out.push("none"),
        }
        out.push(match item.status {
            DeliveryStatus::Pending => "pending",
            DeliveryStatus::Received => "received",
            DeliveryStatus::NotReported => "not_reported",
        });
    }
    put_lot_assumptions(out, &book.assumptions);
    out.push_usize(book.reserved.len());
    for row in &book.reserved {
        put_transaction(out, row);
    }
}
fn put_lot_book(out: &mut Encoder, book: &SavedLotBook) {
    out.push(book.identity.instrument().as_str());
    out.push(book.identity.symbol().as_str());
    put_market(out, book.identity.market());
    put_currency(out, book.identity.currency());
    out.push(match book.identity.cost_currency_kind() {
        LotCostCurrency::Trading => "trading",
        LotCostCurrency::SettlementKrw => "settlement_krw",
    });
    out.push(match book.method {
        CostBasisMethod::Fifo => "fifo",
        CostBasisMethod::MovingAverage => "moving_average",
    });
    out.push_usize(book.lots.len());
    for lot in &book.lots {
        put_date(out, lot.acquired);
        put_share_quantity(out, &lot.quantity);
        put_lot_cost(out, lot.cost);
        out.push(match lot.tax_status {
            LotTaxStatus::Ordinary => "ordinary",
            LotTaxStatus::NeedsConfirmation => "needs_confirmation",
        });
    }
    put_share_quantity(out, &book.pool.quantity);
    put_lot_cost(out, book.pool.cost);
    put_lot_cost(out, book.realized);
    out.push_usize(book.realizations.len());
    for item in &book.realizations {
        put_transaction(out, &item.transaction);
        put_lot_cost(out, item.consumed);
        put_lot_cost(out, item.gain);
        put_lot_assumptions(out, &item.assumptions);
    }
    put_lot_book_corporate(out, book);
}
fn take_lots(input: &mut Decoder<'_>) -> rusqlite::Result<Vec<SavedLot>> {
    let lot_count = input.next_usize()?;
    let mut lots = Vec::with_capacity(lot_count);
    for _ in 0..lot_count {
        let acquired = take_date(input)?;
        let quantity = take_share_quantity(input)?;
        let cost = take_lot_cost(input)?;
        let tax_status = match input.next()? {
            "ordinary" => LotTaxStatus::Ordinary,
            "needs_confirmation" => LotTaxStatus::NeedsConfirmation,
            _ => return Err(invalid(())),
        };
        lots.push(SavedLot {
            acquired,
            quantity,
            cost,
            tax_status,
        });
    }
    Ok(lots)
}
fn take_deliveries(input: &mut Decoder<'_>) -> rusqlite::Result<Vec<SavedCorporateDelivery>> {
    let delivery_count = input.next_usize()?;
    let mut deliveries = Vec::with_capacity(delivery_count);
    for _ in 0..delivery_count {
        let action = CorporateActionId::new(InstrumentId::try_new(input.next()?).map_err(invalid)?);
        let quantity = take_share_quantity(input)?;
        let receipt = match input.next()? {
            "some" => Some(take_transaction(input)?),
            "none" => None,
            _ => return Err(invalid(())),
        };
        let status = match input.next()? {
            "pending" => DeliveryStatus::Pending,
            "received" => DeliveryStatus::Received,
            "not_reported" => DeliveryStatus::NotReported,
            _ => return Err(invalid(())),
        };
        deliveries.push(SavedCorporateDelivery {
            action,
            quantity,
            receipt,
            status,
        });
    }
    Ok(deliveries)
}
fn take_realizations(input: &mut Decoder<'_>) -> rusqlite::Result<Vec<SavedLotRealization>> {
    let count = input.next_usize()?;
    let mut realizations = Vec::with_capacity(count);
    for _ in 0..count {
        realizations.push(SavedLotRealization {
            transaction: take_transaction(input)?,
            consumed: take_lot_cost(input)?,
            gain: take_lot_cost(input)?,
            assumptions: take_lot_assumptions(input)?,
        });
    }
    Ok(realizations)
}
fn take_fractional_entitlements(
    input: &mut Decoder<'_>,
) -> rusqlite::Result<Vec<SavedFractionalEntitlement>> {
    let count = input.next_usize()?;
    let mut fractional = Vec::with_capacity(count);
    for _ in 0..count {
        fractional.push(SavedFractionalEntitlement {
            action: CorporateActionId::new(InstrumentId::try_new(input.next()?).map_err(invalid)?),
            quantity: take_share_quantity(input)?,
            cost: take_lot_cost(input)?,
        });
    }
    Ok(fractional)
}
fn take_lot_book(input: &mut Decoder<'_>) -> rusqlite::Result<SavedLotBook> {
    let instrument = InstrumentId::try_new(input.next()?).map_err(invalid)?;
    let symbol = Symbol::try_new(input.next()?).map_err(invalid)?;
    let market = take_market(input)?;
    let currency = take_currency(input)?;
    let cost_currency = match input.next()? {
        "trading" => LotCostCurrency::Trading,
        "settlement_krw" => LotCostCurrency::SettlementKrw,
        _ => return Err(invalid(())),
    };
    let identity =
        LotIdentity::new(instrument, symbol, market, currency).with_cost_currency(cost_currency);
    let method = match input.next()? {
        "fifo" => CostBasisMethod::Fifo,
        "moving_average" => CostBasisMethod::MovingAverage,
        _ => return Err(invalid(())),
    };
    let lots = take_lots(input)?;
    let pool = SavedCostPool {
        quantity: take_share_quantity(input)?,
        cost: take_lot_cost(input)?,
    };
    let realized = take_lot_cost(input)?;
    let realizations = take_realizations(input)?;
    let listing = match input.next()? {
        "listed" => LotListing::Listed,
        "delisted" => LotListing::Delisted,
        _ => return Err(invalid(())),
    };
    let applied_count = input.next_usize()?;
    let mut applied = BTreeSet::new();
    for _ in 0..applied_count {
        applied.insert(CorporateActionId::new(
            InstrumentId::try_new(input.next()?).map_err(invalid)?,
        ));
    }
    let fractional = take_fractional_entitlements(input)?;
    let deliveries = take_deliveries(input)?;
    let assumptions = take_lot_assumptions(input)?;
    let reserved_count = input.next_usize()?;
    let mut reserved = BTreeSet::new();
    for _ in 0..reserved_count {
        reserved.insert(take_transaction(input)?);
    }
    Ok(SavedLotBook {
        identity,
        method,
        lots,
        pool,
        realized,
        realizations,
        listing,
        applied,
        fractional,
        deliveries,
        assumptions,
        reserved,
    })
}
fn put_flow_period(out: &mut Encoder, period: &SavedConfirmedFlowPeriod) {
    put_date(out, period.window.from());
    put_date(out, period.window.to());
    put_currency(out, period.currency);
    out.push_usize(period.flows.len());
    for flow in &period.flows {
        put_date(out, flow.date);
        match flow.transaction_time {
            Some(time) => {
                out.push("some");
                put_timestamp(out, time);
            }
            None => out.push("none"),
        }
        match flow.exchange_rate {
            Some(rate) => {
                out.push("some");
                put_ratio(out, rate);
            }
            None => out.push("none"),
        }
        out.push(match flow.direction {
            FlowDirection::Inflow => "inflow",
            FlowDirection::Outflow => "outflow",
        });
        put_money(out, flow.amount);
    }
}
fn take_flow_period(input: &mut Decoder<'_>) -> rusqlite::Result<SavedConfirmedFlowPeriod> {
    let from = take_date(input)?;
    let to = take_date(input)?;
    let window = SettlementWindow::try_new(from, to).map_err(invalid)?;
    let currency = take_currency(input)?;
    let count = input.next_usize()?;
    let mut flows = Vec::with_capacity(count);
    for _ in 0..count {
        let date = take_date(input)?;
        let transaction_time = match input.next()? {
            "some" => Some(take_timestamp(input)?),
            "none" => None,
            _ => return Err(invalid(())),
        };
        let exchange_rate = match input.next()? {
            "some" => Some(take_ratio(input)?),
            "none" => None,
            _ => return Err(invalid(())),
        };
        let direction = match input.next()? {
            "inflow" => FlowDirection::Inflow,
            "outflow" => FlowDirection::Outflow,
            _ => return Err(invalid(())),
        };
        let amount = take_money(input)?;
        flows.push(SavedExternalFlow {
            date,
            transaction_time,
            exchange_rate,
            direction,
            amount,
        });
    }
    Ok(SavedConfirmedFlowPeriod {
        window,
        currency,
        flows,
    })
}
fn put_price_basis(out: &mut Encoder, basis: SnapshotPriceBasis) {
    out.push(match basis {
        SnapshotPriceBasis::CurrentSession => "current_session",
        SnapshotPriceBasis::HolidayCarry => "holiday_carry",
        SnapshotPriceBasis::AwaitingSession => "awaiting_session",
    });
}
fn take_price_basis(input: &mut Decoder<'_>) -> rusqlite::Result<SnapshotPriceBasis> {
    match input.next()? {
        "current_session" => Ok(SnapshotPriceBasis::CurrentSession),
        "holiday_carry" => Ok(SnapshotPriceBasis::HolidayCarry),
        "awaiting_session" => Ok(SnapshotPriceBasis::AwaitingSession),
        _ => Err(invalid(())),
    }
}
fn put_cash_rejection(out: &mut Encoder, rejection: CashRequestRejection) {
    out.push(match rejection {
        CashRequestRejection::MalformedRequest => "malformed_request",
        CashRequestRejection::MissingAccountHeader => "missing_account_header",
        CashRequestRejection::InvalidToken => "invalid_token",
        CashRequestRejection::EdgeBlocked => "edge_blocked",
        CashRequestRejection::UnsupportedQuery => "unsupported_query",
        CashRequestRejection::UnsupportedContentType => "unsupported_content_type",
        CashRequestRejection::BatchLimitExceeded => "batch_limit_exceeded",
    });
}
pub(crate) fn put_cash_failure(out: &mut Encoder, failure: CashObservationFailure) {
    match failure {
        CashObservationFailure::AccessDenied(cause) => {
            out.push("access_denied");
            out.push(match cause {
                CashAccessDenial::InvalidCredentials => "invalid_credentials",
                CashAccessDenial::IpNotAllowed => "ip_not_allowed",
                CashAccessDenial::TokenRevokedRepeated => "token_revoked_repeated",
                CashAccessDenial::AccountNotFound => "account_not_found",
                CashAccessDenial::Forbidden => "forbidden",
                CashAccessDenial::LoginUserNotFound => "login_user_not_found",
            });
        }
        CashObservationFailure::Unavailable { cause, retry } => {
            out.push("unavailable");
            out.push(match cause {
                CashUnavailableCause::RateLimited => "rate_limited",
                CashUnavailableCause::Maintenance => "maintenance",
                CashUnavailableCause::ServerError => "server_error",
                CashUnavailableCause::Network => "network",
                CashUnavailableCause::ExpiredToken => "expired_token",
            });
            match retry {
                CashRetryDelay::Unknown => out.push("unknown"),
                CashRetryDelay::Known(span) => {
                    let (seconds, nanos) = span.as_parts();
                    out.push("known");
                    out.push_u64(*seconds.as_u64());
                    out.push_u32(*nanos.as_u32());
                }
            }
        }
        CashObservationFailure::NotFound(missing) => {
            out.push("not_found");
            out.push(match missing {
                CashMissingData::Stock => "stock",
                CashMissingData::ExchangeRate => "exchange_rate",
                CashMissingData::Sector => "sector",
            });
        }
        CashObservationFailure::InvalidResponse(mismatch) => {
            out.push("invalid_response");
            out.push(match mismatch {
                CashResponseMismatch::Schema => "schema",
                CashResponseMismatch::Validation => "validation",
            });
        }
        CashObservationFailure::Unsupported(cap) => {
            out.push("unsupported");
            out.push(match cap {
                CashReadCapability::DailyCandles => "daily_candles",
                CashReadCapability::MinuteCandles => "minute_candles",
                CashReadCapability::AdjustedHistory => "adjusted_history",
                CashReadCapability::RawHistory => "raw_history",
            });
        }
        CashObservationFailure::InvalidRequest(rejection) => {
            out.push("invalid_request");
            put_cash_rejection(out, rejection);
        }
    }
}
fn take_cash_rejection(input: &mut Decoder<'_>) -> rusqlite::Result<CashRequestRejection> {
    match input.next()? {
        "malformed_request" => Ok(CashRequestRejection::MalformedRequest),
        "missing_account_header" => Ok(CashRequestRejection::MissingAccountHeader),
        "invalid_token" => Ok(CashRequestRejection::InvalidToken),
        "edge_blocked" => Ok(CashRequestRejection::EdgeBlocked),
        "unsupported_query" => Ok(CashRequestRejection::UnsupportedQuery),
        "unsupported_content_type" => Ok(CashRequestRejection::UnsupportedContentType),
        "batch_limit_exceeded" => Ok(CashRequestRejection::BatchLimitExceeded),
        _ => Err(invalid(())),
    }
}
fn take_cash_failure_unavailable(
    input: &mut Decoder<'_>,
) -> rusqlite::Result<CashObservationFailure> {
    let cause = match input.next()? {
        "rate_limited" => CashUnavailableCause::RateLimited,
        "maintenance" => CashUnavailableCause::Maintenance,
        "server_error" => CashUnavailableCause::ServerError,
        "network" => CashUnavailableCause::Network,
        "expired_token" => CashUnavailableCause::ExpiredToken,
        _ => return Err(invalid(())),
    };
    let retry = match input.next()? {
        "unknown" => CashRetryDelay::Unknown,
        "known" => CashRetryDelay::Known(
            TimeSpan::try_new(
                SpanSeconds::new(input.next_u64()?),
                Nanosecond::new(input.next_u32()?),
            )
            .map_err(invalid)?,
        ),
        _ => return Err(invalid(())),
    };
    Ok(CashObservationFailure::Unavailable { cause, retry })
}
pub(crate) fn take_cash_failure(
    input: &mut Decoder<'_>,
) -> rusqlite::Result<CashObservationFailure> {
    match input.next()? {
        "access_denied" => Ok(CashObservationFailure::AccessDenied(
            match input.next()? {
                "invalid_credentials" => CashAccessDenial::InvalidCredentials,
                "ip_not_allowed" => CashAccessDenial::IpNotAllowed,
                "token_revoked_repeated" => CashAccessDenial::TokenRevokedRepeated,
                "account_not_found" => CashAccessDenial::AccountNotFound,
                "forbidden" => CashAccessDenial::Forbidden,
                "login_user_not_found" => CashAccessDenial::LoginUserNotFound,
                _ => return Err(invalid(())),
            },
        )),
        "unavailable" => take_cash_failure_unavailable(input),
        "not_found" => Ok(CashObservationFailure::NotFound(match input.next()? {
            "stock" => CashMissingData::Stock,
            "exchange_rate" => CashMissingData::ExchangeRate,
            "sector" => CashMissingData::Sector,
            _ => return Err(invalid(())),
        })),
        "invalid_response" => Ok(CashObservationFailure::InvalidResponse(
            match input.next()? {
                "schema" => CashResponseMismatch::Schema,
                "validation" => CashResponseMismatch::Validation,
                _ => return Err(invalid(())),
            },
        )),
        "unsupported" => Ok(CashObservationFailure::Unsupported(match input.next()? {
            "daily_candles" => CashReadCapability::DailyCandles,
            "minute_candles" => CashReadCapability::MinuteCandles,
            "adjusted_history" => CashReadCapability::AdjustedHistory,
            "raw_history" => CashReadCapability::RawHistory,
            _ => return Err(invalid(())),
        })),
        "invalid_request" => Ok(CashObservationFailure::InvalidRequest(take_cash_rejection(
            input,
        )?)),
        _ => Err(invalid(())),
    }
}
fn put_buying_power(out: &mut Encoder, value: BuyingPower) {
    put_currency(out, value.currency());
    put_timestamp(out, value.observed_at());
    match value.money() {
        Ok(money) => {
            out.push("available");
            put_money(out, money);
        }
        Err(
            PortfolioError::NegativeBuyingPower
            | PortfolioError::TooManyHoldings
            | PortfolioError::DuplicateInstrument
            | PortfolioError::CurrencyMismatch,
        ) => out.push("negative"),
        Err(PortfolioError::CashLookupFailed(reason)) => {
            out.push("failed");
            put_cash_failure(out, reason);
        }
    }
}
fn take_buying_power(input: &mut Decoder<'_>) -> rusqlite::Result<BuyingPower> {
    let currency = take_currency(input)?;
    let observed_at = take_timestamp(input)?;
    match input.next()? {
        "available" => {
            let money = take_money(input)?;
            if money.currency() != currency {
                return Err(invalid(()));
            }
            Ok(BuyingPower::new(money, observed_at))
        }
        "negative" => {
            let neg = Money::parse(&NumericInput::try_new("-1").map_err(invalid)?, currency)
                .map_err(invalid)?;
            Ok(BuyingPower::new(neg, observed_at))
        }
        "failed" => {
            let reason = take_cash_failure(input)?;
            Ok(BuyingPower::from_observation(CashObservation::Failed {
                currency,
                reason,
                observed_at,
            }))
        }
        _ => Err(invalid(())),
    }
}
fn put_snapshot_positions_and_evidence(out: &mut Encoder, saved: &SavedValuationSnapshot) {
    out.push_usize(saved.positions.len());
    for position in &saved.positions {
        out.push(position.instrument.as_str());
        match &position.book {
            Ok(book) => {
                out.push("ok");
                put_lot_book(out, book);
            }
            Err(error) => {
                out.push("err");
                put_lot_error(out, *error);
            }
        }
        match &position.candle {
            Some(candle) => {
                out.push("some");
                put_candle(out, candle);
            }
            None => out.push("none"),
        }
        put_money_result(out, position.value);
    }
    out.push_usize(saved.observed_close_evidence.len());
    for entry in &saved.observed_close_evidence {
        let quote_saved = entry.instrument().saved_state();
        put_known_market(out, quote_saved.market);
        put_currency(out, quote_saved.currency);
        out.push(quote_saved.symbol.as_str());
        out.push(quote_saved.instrument_id.as_str());
        put_candle(out, entry.candle());
    }
}
fn put_snapshot_body(out: &mut Encoder, snapshot: &ValuationSnapshot) {
    let saved = snapshot.saved_state();
    put_date(out, saved.dates.date(Currency::Krw));
    put_date(out, saved.dates.date(Currency::Usd));
    put_date(out, saved.price_dates.date(Currency::Krw));
    put_date(out, saved.price_dates.date(Currency::Usd));
    put_price_basis(out, saved.kr_basis);
    put_price_basis(out, saved.us_basis);
    put_date(out, saved.settlement_date);
    put_timestamp(out, saved.valuation_at);
    put_timestamp(out, saved.recorded_at);
    match saved.fx {
        Some(rate) => {
            out.push("some");
            put_fx_rate(out, rate);
        }
        None => out.push("none"),
    }
    out.push(match saved.source {
        SnapshotSource::Observed => "observed",
        SnapshotSource::Recalculated => "recalculated",
    });
    put_money_result(out, saved.stock_krw);
    put_money_result(out, saved.stock_usd);
    out.push(match saved.cash_basis {
        SnapshotCashBasis::OrderBuyingPower => "order_buying_power",
        SnapshotCashBasis::SettledLedger => "settled_ledger",
        SnapshotCashBasis::Unavailable => "unavailable",
    });
    put_money_result(out, saved.cash_krw);
    put_money_result(out, saved.cash_usd);
    match saved.buying_power {
        Some((kr, us)) => {
            out.push("some");
            put_buying_power(out, kr);
            put_buying_power(out, us);
        }
        None => out.push("none"),
    }
    match &saved.periods {
        Some((kr, us)) => {
            out.push("some");
            put_flow_period(out, kr);
            put_flow_period(out, us);
        }
        None => out.push("none"),
    }
    out.push_usize(saved.exchanges.len());
    for row in &saved.exchanges {
        put_transaction(out, row);
    }
    put_money_result(out, saved.reported_krw);
    put_money_result(out, saved.reported_usd);
    put_snapshot_positions_and_evidence(out, &saved);
}
fn take_snapshot_positions(
    input: &mut Decoder<'_>,
) -> rusqlite::Result<Vec<SavedSnapshotPosition>> {
    let position_count = input.next_usize()?;
    let mut positions = Vec::with_capacity(position_count);
    for _ in 0..position_count {
        let instrument = InstrumentId::try_new(input.next()?).map_err(invalid)?;
        let book = match input.next()? {
            "ok" => Ok(take_lot_book(input)?),
            "err" => Err(take_lot_error(input)?),
            _ => return Err(invalid(())),
        };
        let candle = match input.next()? {
            "some" => Some(take_candle(input)?),
            "none" => None,
            _ => return Err(invalid(())),
        };
        let value = take_money_result(input)?;
        positions.push(SavedSnapshotPosition {
            instrument,
            book,
            candle,
            value,
        });
    }
    Ok(positions)
}
fn take_close_evidence(input: &mut Decoder<'_>) -> rusqlite::Result<Vec<ObservedCloseEvidence>> {
    let evidence_count = input.next_usize()?;
    let mut observed_close_evidence = Vec::with_capacity(evidence_count);
    for _ in 0..evidence_count {
        let market = take_known_market(input)?;
        let currency = take_currency(input)?;
        let symbol = Symbol::try_new(input.next()?).map_err(invalid)?;
        let instrument_id = InstrumentId::try_new(input.next()?).map_err(invalid)?;
        let quote_instrument = QuoteInstrument::restore_from_saved_state(SavedQuoteInstrument {
            market,
            currency,
            symbol,
            instrument_id,
        })
        .map_err(invalid)?;
        let candle = take_candle(input)?;
        observed_close_evidence.push(ObservedCloseEvidence::new(quote_instrument, candle));
    }
    Ok(observed_close_evidence)
}
fn take_optional_pair<T>(
    input: &mut Decoder<'_>,
    mut parse: impl FnMut(&mut Decoder<'_>) -> rusqlite::Result<T>,
) -> rusqlite::Result<Option<(T, T)>> {
    match input.next()? {
        "some" => Ok(Some((parse(input)?, parse(input)?))),
        "none" => Ok(None),
        _ => Err(invalid(())),
    }
}
fn take_exchanges(input: &mut Decoder<'_>) -> rusqlite::Result<Vec<Transaction>> {
    let count = input.next_usize()?;
    let mut exchanges = Vec::with_capacity(count);
    for _ in 0..count {
        exchanges.push(take_transaction(input)?);
    }
    Ok(exchanges)
}
fn take_snapshot_body(input: &mut Decoder<'_>) -> rusqlite::Result<ValuationSnapshot> {
    let dates = SnapshotDates::new(take_date(input)?, take_date(input)?);
    let price_dates = SnapshotDates::new(take_date(input)?, take_date(input)?);
    let kr_basis = take_price_basis(input)?;
    let us_basis = take_price_basis(input)?;
    let settlement_date = take_date(input)?;
    let valuation_at = take_timestamp(input)?;
    let recorded_at = take_timestamp(input)?;
    let fx = match input.next()? {
        "some" => Some(take_fx_rate(input)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    let source = match input.next()? {
        "observed" => SnapshotSource::Observed,
        "recalculated" => SnapshotSource::Recalculated,
        _ => return Err(invalid(())),
    };
    let stock_krw = take_money_result(input)?;
    let stock_usd = take_money_result(input)?;
    let cash_basis = match input.next()? {
        "order_buying_power" => SnapshotCashBasis::OrderBuyingPower,
        "settled_ledger" => SnapshotCashBasis::SettledLedger,
        "unavailable" => SnapshotCashBasis::Unavailable,
        _ => return Err(invalid(())),
    };
    let cash_krw = take_money_result(input)?;
    let cash_usd = take_money_result(input)?;
    let buying_power = take_optional_pair(input, take_buying_power)?;
    let periods = take_optional_pair(input, take_flow_period)?;
    let exchanges = take_exchanges(input)?;
    let reported_krw = take_money_result(input)?;
    let reported_usd = take_money_result(input)?;
    let positions = take_snapshot_positions(input)?;
    let observed_close_evidence = take_close_evidence(input)?;
    ValuationSnapshot::restore_from_saved_state(SavedValuationSnapshot {
        dates,
        price_dates,
        kr_basis,
        us_basis,
        settlement_date,
        valuation_at,
        recorded_at,
        fx,
        source,
        stock_krw,
        stock_usd,
        cash_basis,
        cash_krw,
        cash_usd,
        buying_power,
        periods,
        exchanges,
        reported_krw,
        reported_usd,
        positions,
        observed_close_evidence,
    })
    .map_err(invalid)
}
pub(super) fn encode_valuation_snapshot(snapshot: &ValuationSnapshot) -> String {
    let mut out = Encoder::new();
    out.push("snapshot_v1");
    put_snapshot_body(&mut out, snapshot);
    out.finish()
}
pub(super) fn decode_valuation_snapshot(payload: &str) -> rusqlite::Result<ValuationSnapshot> {
    let mut input = Decoder::new(payload);
    if input.next()? != "snapshot_v1" {
        return Err(invalid(()));
    }
    let snapshot = take_snapshot_body(&mut input)?;
    input.finish()?;
    Ok(snapshot)
}
pub(super) fn encode_valuation_day(result: &ValuationDayResult) -> String {
    let mut out = Encoder::new();
    out.push("day_result_v1");
    put_date(&mut out, result.date());
    match result.result() {
        Ok(snapshot) => {
            out.push("ok");
            put_snapshot_body(&mut out, snapshot);
        }
        Err(error) => {
            out.push("err");
            put_snapshot_error(&mut out, *error);
        }
    }
    out.finish()
}
pub(super) fn decode_valuation_day(payload: &str) -> rusqlite::Result<ValuationDayResult> {
    let mut input = Decoder::new(payload);
    if input.next()? != "day_result_v1" {
        return Err(invalid(()));
    }
    let date = take_date(&mut input)?;
    let outcome = match input.next()? {
        "ok" => Ok(take_snapshot_body(&mut input)?),
        "err" => Err(take_snapshot_error(&mut input)?),
        _ => return Err(invalid(())),
    };
    input.finish()?;
    ValuationDayResult::restore_from_saved_state(date, outcome).map_err(invalid)
}
