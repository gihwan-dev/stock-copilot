use super::context_codec::{
    put_collected_item, put_disclosure, put_hours_permission, put_signal_context,
    put_stored_price_obs, take_collected_item, take_disclosure, take_hours_permission,
    take_signal_context, take_stored_price_obs,
};
use crate::estimates::codec::{
    Decoder, Encoder, decode_calibration, encode_calibration, invalid, put_ai_probability,
    put_appetite, put_calibration_key, put_currency, put_date, put_evidence, put_horizon,
    put_interval, put_price, put_ratio, put_timestamp, take_ai_probability, take_appetite,
    take_calibration_key, take_currency, take_date, take_evidence, take_horizon, take_interval,
    take_price, take_ratio, take_timestamp,
};
use crate::estimates::valuation_codec::{put_candle, put_market, take_candle, take_market};
use application::{SignalCommitEvidence, SignalSourceFacts};
use domain::{
    AccountId, BuySignalOccurrenceId, CalibrationLedgerHead, CalibrationPosition,
    CalibrationSnapshot, ClientOrderId, CommissionRateFact, CommissionValidity, CommonShare,
    CostRate, Currency, DisplayInstrumentName, DraftOriginalFacts, DraftRestoreEvidence,
    EstimateDisplayFacts, EstimateRestoreEvidence, HistoricalOrderAmount, HistoricalOrderTerms,
    HistoricalSellQuantity, HistoricalSignalDirection, HistoricalSignalLimit,
    HistoricalSignalQuantity, HistoricalWholeQuantity, InstrumentId, KoreanMasterDetail,
    KrOrderPrice, LiquidationTrading, ListingStatus, Margin, MasterCode, MasterInstrument,
    MasterInstrumentRow, MasterSecurityType, Money, NumericInput, NxtTrading, OrderSide,
    Probability, ProposalId, ProposalLifetime, Quantity, SignalDisplayFacts, SignalOperation,
    SignalRestoreEvidence, SpanSeconds, StoredEstimateEvidence, StoredSignalEvidence,
    StoredSignalOccurrence, Symbol, ToolReference, TradeTaxFact, TradingDayHorizon,
    TradingSuspension, UsOrderPrice,
};

fn put_order_side(out: &mut Encoder, side: OrderSide) {
    out.push(match side {
        OrderSide::Buy => "buy",
        OrderSide::Sell => "sell",
    });
}

fn take_order_side(input: &mut Decoder<'_>) -> rusqlite::Result<OrderSide> {
    match input.next()? {
        "buy" => Ok(OrderSide::Buy),
        "sell" => Ok(OrderSide::Sell),
        _ => Err(invalid(())),
    }
}

fn put_korean_detail(out: &mut Encoder, detail: KoreanMasterDetail) {
    out.push(match detail.liquidation {
        LiquidationTrading::Regular => "regular",
        LiquidationTrading::Liquidation => "liquidation",
    });
    out.push(match detail.krx {
        TradingSuspension::Clear => "clear",
        TradingSuspension::Suspended => "suspended",
    });
    out.push(match detail.nxt {
        NxtTrading::Unsupported => "unsupported",
        NxtTrading::Supported(TradingSuspension::Clear) => "supported_clear",
        NxtTrading::Supported(TradingSuspension::Suspended) => "supported_suspended",
    });
}

fn take_korean_detail(input: &mut Decoder<'_>) -> rusqlite::Result<KoreanMasterDetail> {
    let liquidation = match input.next()? {
        "regular" => LiquidationTrading::Regular,
        "liquidation" => LiquidationTrading::Liquidation,
        _ => return Err(invalid(())),
    };
    let krx = match input.next()? {
        "clear" => TradingSuspension::Clear,
        "suspended" => TradingSuspension::Suspended,
        _ => return Err(invalid(())),
    };
    let nxt = match input.next()? {
        "unsupported" => NxtTrading::Unsupported,
        "supported_clear" => NxtTrading::Supported(TradingSuspension::Clear),
        "supported_suspended" => NxtTrading::Supported(TradingSuspension::Suspended),
        _ => return Err(invalid(())),
    };
    Ok(KoreanMasterDetail {
        liquidation,
        krx,
        nxt,
    })
}

fn put_master_row(out: &mut Encoder, row: &MasterInstrumentRow) {
    out.push(row.listing_market.as_str());
    out.push(row.symbol.as_str());
    out.push(row.instrument_id.as_str());
    out.push(row.name.as_str());
    out.push(row.security_type.code().as_str());
    put_currency(out, row.currency);
    out.push(match row.listing_status {
        ListingStatus::Scheduled => "scheduled",
        ListingStatus::Active => "active",
        ListingStatus::Delisted => "delisted",
    });
    out.push(match row.common_share {
        CommonShare::Common => "common",
        CommonShare::Other => "other",
    });
    match row.korean_detail {
        Some(detail) => {
            out.push("some");
            put_korean_detail(out, detail);
        }
        None => out.push("none"),
    }
    put_timestamp(out, row.observed_at);
}

fn take_master_row(input: &mut Decoder<'_>) -> rusqlite::Result<MasterInstrumentRow> {
    let listing_market = MasterCode::try_new(input.next()?).map_err(invalid)?;
    let symbol = Symbol::try_new(input.next()?).map_err(invalid)?;
    let instrument_id = InstrumentId::try_new(input.next()?).map_err(invalid)?;
    let name = DisplayInstrumentName::try_new(input.next()?).map_err(invalid)?;
    let security_type =
        MasterSecurityType::from_code(MasterCode::try_new(input.next()?).map_err(invalid)?);
    let currency = take_currency(input)?;
    let listing_status = match input.next()? {
        "scheduled" => ListingStatus::Scheduled,
        "active" => ListingStatus::Active,
        "delisted" => ListingStatus::Delisted,
        _ => return Err(invalid(())),
    };
    let common_share = match input.next()? {
        "common" => CommonShare::Common,
        "other" => CommonShare::Other,
        _ => return Err(invalid(())),
    };
    let korean_detail = match input.next()? {
        "some" => Some(take_korean_detail(input)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    let observed_at = take_timestamp(input)?;
    Ok(MasterInstrumentRow {
        listing_market,
        symbol,
        instrument_id,
        name,
        security_type,
        currency,
        listing_status,
        common_share,
        korean_detail,
        observed_at,
    })
}

pub(super) fn encode_master(master: &MasterInstrument) -> String {
    let mut out = Encoder::new();
    out.push("master_v1");
    put_master_row(&mut out, master.row());
    out.finish()
}

pub(super) fn decode_master(payload: &str) -> rusqlite::Result<MasterInstrument> {
    let mut input = Decoder::new(payload);
    if input.next()? != "master_v1" {
        return Err(invalid(()));
    }
    let row = take_master_row(&mut input)?;
    input.finish()?;
    MasterInstrument::try_new(row).map_err(invalid)
}

pub(super) fn encode_sources(facts: &SignalSourceFacts) -> String {
    let mut out = Encoder::new();
    out.push("sources_v1");
    out.push_usize(facts.items.len());
    for item in &facts.items {
        put_collected_item(&mut out, item);
    }
    out.push_usize(facts.disclosures.len());
    for disclosure in &facts.disclosures {
        put_disclosure(&mut out, disclosure);
    }
    out.push_usize(facts.tools.len());
    for tool in &facts.tools {
        out.push(tool.as_str());
    }
    out.finish()
}

pub(super) fn decode_sources(payload: &str) -> rusqlite::Result<SignalSourceFacts> {
    let mut input = Decoder::new(payload);
    if input.next()? != "sources_v1" {
        return Err(invalid(()));
    }
    let item_count = input.next_usize()?;
    let mut items = Vec::with_capacity(item_count);
    for _ in 0..item_count {
        items.push(take_collected_item(&mut input)?);
    }
    let disclosure_count = input.next_usize()?;
    let mut disclosures = Vec::with_capacity(disclosure_count);
    for _ in 0..disclosure_count {
        disclosures.push(take_disclosure(&mut input)?);
    }
    let tool_count = input.next_usize()?;
    let mut tools = Vec::with_capacity(tool_count);
    for _ in 0..tool_count {
        tools.push(ToolReference::try_new(input.next()?).map_err(invalid)?);
    }
    input.finish()?;
    Ok(SignalSourceFacts {
        items,
        disclosures,
        tools,
    })
}

fn put_estimate_restore_evidence(out: &mut Encoder, value: &EstimateRestoreEvidence) {
    put_master_row(out, &value.master);
    put_order_side(out, value.side);
    out.push_usize(value.candles.len());
    for candle in &value.candles {
        put_candle(out, candle);
    }
    put_price(out, value.reference_price);
    put_timestamp(out, value.reference_at);
    put_timestamp(out, value.task_at);
    put_horizon(out, value.horizon);
    put_hours_permission(out, value.permission);
    put_appetite(out, value.appetite);
}

fn take_estimate_restore_evidence(
    input: &mut Decoder<'_>,
) -> rusqlite::Result<EstimateRestoreEvidence> {
    let master = take_master_row(input)?;
    let side = take_order_side(input)?;
    let candle_count = input.next_usize()?;
    let mut candles = Vec::with_capacity(candle_count);
    for _ in 0..candle_count {
        candles.push(take_candle(input)?);
    }
    let reference_price = take_price(input)?;
    let reference_at = take_timestamp(input)?;
    let task_at = take_timestamp(input)?;
    let horizon = take_horizon(input)?;
    let permission = take_hours_permission(input)?;
    let appetite = take_appetite(input)?;
    Ok(EstimateRestoreEvidence {
        master,
        side,
        candles,
        reference_price,
        reference_at,
        task_at,
        horizon,
        permission,
        appetite,
    })
}

fn put_stored_estimate_evidence(out: &mut Encoder, value: &StoredEstimateEvidence) {
    put_estimate_restore_evidence(out, &value.evidence);
    put_estimate_restore_evidence(out, &value.original);
    put_price(out, value.expected.reference_price);
    put_timestamp(out, value.expected.reference_at);
    put_timestamp(out, value.expected.created_at);
    put_ratio(out, value.expected.horizon.as_ratio());
    put_ratio(out, value.expected.target);
    put_ratio(out, value.expected.stop);
    put_ratio(out, value.expected.base_probability);
}

fn take_stored_estimate_evidence(
    input: &mut Decoder<'_>,
) -> rusqlite::Result<StoredEstimateEvidence> {
    let evidence = take_estimate_restore_evidence(input)?;
    let original = take_estimate_restore_evidence(input)?;
    let reference_price = take_price(input)?;
    let reference_at = take_timestamp(input)?;
    let created_at = take_timestamp(input)?;
    let horizon = TradingDayHorizon::try_new(take_ratio(input)?).map_err(invalid)?;
    let target = take_ratio(input)?;
    let stop = take_ratio(input)?;
    let base_probability = take_ratio(input)?;
    Ok(StoredEstimateEvidence {
        evidence,
        original,
        expected: EstimateDisplayFacts {
            reference_price,
            reference_at,
            created_at,
            horizon,
            target,
            stop,
            base_probability,
        },
    })
}

fn put_order_terms(out: &mut Encoder, terms: HistoricalOrderTerms) {
    match terms {
        HistoricalOrderTerms::KrLimit {
            side,
            quantity,
            price,
        } => {
            out.push("kr_limit");
            put_order_side(out, side);
            out.push(quantity.display_text().as_str());
            put_price(out, price.price());
        }
        HistoricalOrderTerms::KrMarket { side, quantity } => {
            out.push("kr_market");
            put_order_side(out, side);
            out.push(quantity.display_text().as_str());
        }
        HistoricalOrderTerms::UsLimit {
            side,
            quantity,
            price,
        } => {
            out.push("us_limit");
            put_order_side(out, side);
            out.push(quantity.display_text().as_str());
            put_price(out, price.price());
        }
        HistoricalOrderTerms::UsMarketBuy { quantity } => {
            out.push("us_market_buy");
            out.push(quantity.display_text().as_str());
        }
        HistoricalOrderTerms::UsMarketSell { quantity } => {
            out.push("us_market_sell");
            out.push(quantity.display_text().as_str());
        }
        HistoricalOrderTerms::UsMarketAmountBuy { amount } => {
            out.push("us_market_amount_buy");
            out.push(amount.display_text().as_str());
        }
    }
}

fn take_whole_qty(input: &mut Decoder<'_>) -> rusqlite::Result<HistoricalWholeQuantity> {
    let parsed = Quantity::parse(&NumericInput::try_new(input.next()?).map_err(invalid)?)
        .map_err(invalid)?;
    HistoricalWholeQuantity::try_new(parsed).map_err(invalid)
}

fn take_order_terms(input: &mut Decoder<'_>) -> rusqlite::Result<HistoricalOrderTerms> {
    match input.next()? {
        "kr_limit" => Ok(HistoricalOrderTerms::KrLimit {
            side: take_order_side(input)?,
            quantity: take_whole_qty(input)?,
            price: KrOrderPrice::try_new(take_price(input)?).map_err(invalid)?,
        }),
        "kr_market" => Ok(HistoricalOrderTerms::KrMarket {
            side: take_order_side(input)?,
            quantity: take_whole_qty(input)?,
        }),
        "us_limit" => Ok(HistoricalOrderTerms::UsLimit {
            side: take_order_side(input)?,
            quantity: take_whole_qty(input)?,
            price: UsOrderPrice::try_new(take_price(input)?).map_err(invalid)?,
        }),
        "us_market_buy" => Ok(HistoricalOrderTerms::UsMarketBuy {
            quantity: take_whole_qty(input)?,
        }),
        "us_market_sell" => {
            let parsed = Quantity::parse(&NumericInput::try_new(input.next()?).map_err(invalid)?)
                .map_err(invalid)?;
            Ok(HistoricalOrderTerms::UsMarketSell {
                quantity: HistoricalSellQuantity::try_new(parsed).map_err(invalid)?,
            })
        }
        "us_market_amount_buy" => {
            let money = Money::parse(
                &NumericInput::try_new(input.next()?).map_err(invalid)?,
                Currency::Usd,
            )
            .map_err(invalid)?;
            Ok(HistoricalOrderTerms::UsMarketAmountBuy {
                amount: HistoricalOrderAmount::try_new(money).map_err(invalid)?,
            })
        }
        _ => Err(invalid(())),
    }
}

pub(super) fn encode_draft_evidence(evidence: &DraftRestoreEvidence) -> String {
    let mut out = Encoder::new();
    out.push("draft_v1");
    put_stored_estimate_evidence(&mut out, &evidence.estimate);
    out.push(evidence.id.as_client_order_id().as_str());
    out.push_i64(evidence.account.as_i64());
    put_order_terms(&mut out, evidence.terms);
    put_timestamp(&mut out, evidence.created_at);
    out.push_u64(*evidence.lifetime.as_seconds().as_u64());
    put_timestamp(&mut out, evidence.expires_at);
    put_stored_price_obs(&mut out, evidence.card);
    put_stored_price_obs(&mut out, evidence.expected_card);
    out.push(evidence.original.id.as_client_order_id().as_str());
    out.push_i64(evidence.original.account.as_i64());
    put_order_terms(&mut out, evidence.original.terms);
    put_timestamp(&mut out, evidence.original.created_at);
    out.push_u64(*evidence.original.lifetime.as_seconds().as_u64());
    out.finish()
}

pub(super) fn decode_draft_evidence(payload: &str) -> rusqlite::Result<DraftRestoreEvidence> {
    let mut input = Decoder::new(payload);
    if input.next()? != "draft_v1" {
        return Err(invalid(()));
    }
    let estimate = take_stored_estimate_evidence(&mut input)?;
    let id = ProposalId::new(ClientOrderId::try_new(input.next()?).map_err(invalid)?);
    let account = AccountId::new(input.next_i64()?);
    let terms = take_order_terms(&mut input)?;
    let created_at = take_timestamp(&mut input)?;
    let lifetime =
        ProposalLifetime::try_new(SpanSeconds::new(input.next_u64()?)).map_err(invalid)?;
    let expires_at = take_timestamp(&mut input)?;
    let card = take_stored_price_obs(&mut input)?;
    let expected_card = take_stored_price_obs(&mut input)?;
    let orig_id = ProposalId::new(ClientOrderId::try_new(input.next()?).map_err(invalid)?);
    let orig_account = AccountId::new(input.next_i64()?);
    let orig_terms = take_order_terms(&mut input)?;
    let orig_created_at = take_timestamp(&mut input)?;
    let orig_lifetime =
        ProposalLifetime::try_new(SpanSeconds::new(input.next_u64()?)).map_err(invalid)?;
    input.finish()?;
    Ok(DraftRestoreEvidence {
        estimate,
        id,
        account,
        terms,
        created_at,
        lifetime,
        expires_at,
        card,
        expected_card,
        original: DraftOriginalFacts {
            id: orig_id,
            account: orig_account,
            terms: orig_terms,
            created_at: orig_created_at,
            lifetime: orig_lifetime,
        },
    })
}

fn put_signal_restore_evidence(out: &mut Encoder, value: &SignalRestoreEvidence) {
    put_stored_estimate_evidence(out, &value.estimate);
    put_market(out, value.commission.market());
    put_ratio(out, value.commission.rate().ratio());
    match value.commission.validity().start() {
        Some(start) => {
            out.push("some");
            put_date(out, start);
        }
        None => out.push("none"),
    }
    match value.commission.validity().end() {
        Some(end) => {
            out.push("some");
            put_date(out, end);
        }
        None => out.push("none"),
    }
    put_market(out, value.tax.market());
    put_ratio(out, value.tax.buy_rate().ratio());
    put_ratio(out, value.tax.sell_rate().ratio());
    put_timestamp(out, value.maturity);
    out.push(&encode_calibration(value.calibration.calibration()));
    put_calibration_key(out, value.calibration.through().key());
    out.push_u64(*value.calibration.through().position().as_u64());
    put_calibration_key(out, &value.calibration_key);
    put_ai_probability(out, value.probability);
    match &value.source {
        Some(source) => {
            out.push("some");
            put_evidence(out, source);
        }
        None => out.push("none"),
    }
    put_timestamp(out, value.submitted_at);
    put_ratio(out, value.margin.ratio());
    put_signal_context(out, &value.assessment);
}

fn take_cost_facts(
    input: &mut Decoder<'_>,
) -> rusqlite::Result<(CommissionRateFact, TradeTaxFact)> {
    let comm_market = take_market(input)?;
    let comm_rate = CostRate::try_new(take_ratio(input)?).map_err(invalid)?;
    let start = match input.next()? {
        "some" => Some(take_date(input)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    let end = match input.next()? {
        "some" => Some(take_date(input)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    let validity = CommissionValidity::try_new(start, end).map_err(invalid)?;
    let commission = CommissionRateFact::from_commissions_result(comm_market, comm_rate, validity);
    let tax_market = take_market(input)?;
    let buy = CostRate::try_new(take_ratio(input)?).map_err(invalid)?;
    let sell = CostRate::try_new(take_ratio(input)?).map_err(invalid)?;
    let tax = TradeTaxFact::from_schedule_result(tax_market, buy, sell);
    Ok((commission, tax))
}

fn take_signal_restore_evidence(
    input: &mut Decoder<'_>,
    masters: &[MasterInstrument],
) -> rusqlite::Result<SignalRestoreEvidence> {
    let estimate = take_stored_estimate_evidence(input)?;
    let (commission, tax) = take_cost_facts(input)?;
    let maturity = take_timestamp(input)?;
    let cal_table = decode_calibration(input.next()?)?;
    let through_key = take_calibration_key(input)?;
    let through_pos = CalibrationPosition::new(input.next_u64()?);
    let calibration = CalibrationSnapshot::try_new(
        cal_table,
        CalibrationLedgerHead::new(through_key, through_pos),
    )
    .map_err(invalid)?;
    let calibration_key = take_calibration_key(input)?;
    let probability = take_ai_probability(input)?;
    let source = match input.next()? {
        "some" => Some(take_evidence(input)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    let submitted_at = take_timestamp(input)?;
    let margin = Margin::try_new(take_ratio(input)?).map_err(invalid)?;
    let assessment = take_signal_context(input, masters)?;
    Ok(SignalRestoreEvidence {
        estimate,
        commission,
        tax,
        maturity,
        calibration,
        calibration_key,
        probability,
        source,
        submitted_at,
        margin,
        assessment,
    })
}

fn put_signal_display(out: &mut Encoder, facts: &SignalDisplayFacts) {
    match facts.direction {
        HistoricalSignalDirection::Buy { limit } => {
            out.push("buy");
            put_currency(out, limit.currency());
            out.push(limit.display_text().as_str());
        }
        HistoricalSignalDirection::Sell {
            quantity,
            observed_at,
        } => {
            out.push("sell");
            out.push(quantity.display_text().as_str());
            put_timestamp(out, observed_at);
        }
    }
    out.push(&facts.probability.as_decimal().to_string());
    out.push(match facts.operation {
        SignalOperation::Conservative => "conservative",
        SignalOperation::EligibleBeforeE91 => "eligible_before_e91",
    });
    put_ratio(out, facts.buy_threshold);
    put_ratio(out, facts.sell_threshold);
    put_ratio(out, facts.breakeven_threshold);
}

fn take_signal_display(input: &mut Decoder<'_>) -> rusqlite::Result<SignalDisplayFacts> {
    let direction = match input.next()? {
        "buy" => {
            let currency = take_currency(input)?;
            let numeric = NumericInput::try_new(input.next()?).map_err(invalid)?;
            let money = Money::parse(&numeric, currency).map_err(invalid)?;
            HistoricalSignalDirection::Buy {
                limit: HistoricalSignalLimit::new(money),
            }
        }
        "sell" => {
            let qty = Quantity::parse(&NumericInput::try_new(input.next()?).map_err(invalid)?)
                .map_err(invalid)?;
            let observed_at = take_timestamp(input)?;
            HistoricalSignalDirection::Sell {
                quantity: HistoricalSignalQuantity::new(qty),
                observed_at,
            }
        }
        _ => return Err(invalid(())),
    };
    let prob_ratio = take_ratio(input)?;
    let probability = Probability::try_new(prob_ratio.as_decimal()).map_err(invalid)?;
    let operation = match input.next()? {
        "conservative" => SignalOperation::Conservative,
        "eligible_before_e91" => SignalOperation::EligibleBeforeE91,
        _ => return Err(invalid(())),
    };
    Ok(SignalDisplayFacts {
        direction,
        probability,
        operation,
        buy_threshold: take_ratio(input)?,
        sell_threshold: take_ratio(input)?,
        breakeven_threshold: take_ratio(input)?,
    })
}

fn put_stored_signal_evidence(out: &mut Encoder, value: &StoredSignalEvidence) {
    put_signal_restore_evidence(out, &value.evidence);
    put_signal_restore_evidence(out, &value.original);
    put_signal_display(out, &value.expected);
}

fn take_stored_signal_evidence(
    input: &mut Decoder<'_>,
    masters: &[MasterInstrument],
) -> rusqlite::Result<StoredSignalEvidence> {
    let evidence = take_signal_restore_evidence(input, masters)?;
    let original = take_signal_restore_evidence(input, masters)?;
    let expected = take_signal_display(input)?;
    Ok(StoredSignalEvidence {
        evidence,
        original,
        expected,
    })
}

fn put_stored_occurrence(out: &mut Encoder, value: &StoredSignalOccurrence) {
    put_stored_signal_evidence(out, &value.signal);
    out.push(value.id.as_str());
    out.push(value.expected_id.as_str());
    put_timestamp(out, value.raised_at);
    put_interval(out, value.interval);
}

fn take_stored_occurrence(
    input: &mut Decoder<'_>,
    masters: &[MasterInstrument],
) -> rusqlite::Result<StoredSignalOccurrence> {
    let signal = take_stored_signal_evidence(input, masters)?;
    let id = BuySignalOccurrenceId::try_new(input.next()?).map_err(invalid)?;
    let expected_id = BuySignalOccurrenceId::try_new(input.next()?).map_err(invalid)?;
    let raised_at = take_timestamp(input)?;
    let interval = take_interval(input)?;
    Ok(StoredSignalOccurrence {
        signal,
        id,
        expected_id,
        raised_at,
        interval,
    })
}

pub(super) fn encode_stored_occurrence(value: &StoredSignalOccurrence) -> String {
    let mut out = Encoder::new();
    out.push("occurrence_v1");
    put_stored_occurrence(&mut out, value);
    out.finish()
}

pub(super) fn decode_stored_occurrence(
    payload: &str,
    masters: &[MasterInstrument],
) -> rusqlite::Result<StoredSignalOccurrence> {
    let mut input = Decoder::new(payload);
    if input.next()? != "occurrence_v1" {
        return Err(invalid(()));
    }
    let value = take_stored_occurrence(&mut input, masters)?;
    input.finish()?;
    Ok(value)
}

#[derive(Debug, Clone)]
pub(super) struct StoredCurrentEntry {
    pub(super) submission_id: String,
    pub(super) occurrence_id: Option<BuySignalOccurrenceId>,
    pub(super) evidence: SignalCommitEvidence,
}

pub(super) fn encode_current_entry(entry: &StoredCurrentEntry) -> String {
    let mut out = Encoder::new();
    out.push("current_v1");
    out.push(&entry.submission_id);
    match &entry.occurrence_id {
        Some(id) => {
            out.push("some");
            out.push(id.as_str());
        }
        None => out.push("none"),
    }
    put_master_row(&mut out, entry.evidence.master.row());
    out.push_usize(entry.evidence.holding_masters.len());
    for extra in &entry.evidence.holding_masters {
        put_master_row(&mut out, extra.row());
    }
    put_stored_signal_evidence(&mut out, &entry.evidence.signal);
    match &entry.evidence.occurrence {
        Some(occ) => {
            out.push("some");
            put_stored_occurrence(&mut out, occ);
        }
        None => out.push("none"),
    }
    out.finish()
}

pub(super) fn decode_current_entry(
    payload: &str,
    masters: &[MasterInstrument],
) -> rusqlite::Result<StoredCurrentEntry> {
    let mut input = Decoder::new(payload);
    if input.next()? != "current_v1" {
        return Err(invalid(()));
    }
    let submission_id = input.next()?.to_owned();
    let occurrence_id = match input.next()? {
        "some" => Some(BuySignalOccurrenceId::try_new(input.next()?).map_err(invalid)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    let master = MasterInstrument::try_new(take_master_row(&mut input)?).map_err(invalid)?;
    let extra_count = input.next_usize()?;
    let mut holding_masters = Vec::with_capacity(extra_count);
    for _ in 0..extra_count {
        holding_masters
            .push(MasterInstrument::try_new(take_master_row(&mut input)?).map_err(invalid)?);
    }
    let mut combined = masters.to_vec();
    for item in std::iter::once(&master).chain(&holding_masters) {
        if !combined
            .iter()
            .any(|seen| seen.row().instrument_id == item.row().instrument_id)
        {
            combined.push(item.clone());
        }
    }
    let signal = take_stored_signal_evidence(&mut input, &combined)?;
    let occurrence = match input.next()? {
        "some" => Some(take_stored_occurrence(&mut input, &combined)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    input.finish()?;
    Ok(StoredCurrentEntry {
        submission_id,
        occurrence_id,
        evidence: SignalCommitEvidence {
            master,
            holding_masters,
            signal,
            occurrence,
        },
    })
}

#[derive(Debug, Clone)]
pub(super) struct StoredSnapshotBinding {
    pub(super) revision: u64,
    pub(super) submission_id: Option<String>,
    pub(super) current_payload: Option<String>,
}

pub(super) fn encode_snapshot_binding(binding: &StoredSnapshotBinding) -> String {
    let mut out = Encoder::new();
    out.push("snap_v1");
    out.push_u64(binding.revision);
    match &binding.submission_id {
        Some(id) => {
            out.push("some");
            out.push(id);
        }
        None => out.push("none"),
    }
    match &binding.current_payload {
        Some(payload) => {
            out.push("some");
            out.push(payload);
        }
        None => out.push("none"),
    }
    out.finish()
}

pub(super) fn decode_snapshot_binding(payload: &str) -> rusqlite::Result<StoredSnapshotBinding> {
    let mut input = Decoder::new(payload);
    if input.next()? != "snap_v1" {
        return Err(invalid(()));
    }
    let revision = input.next_u64()?;
    let submission_id = match input.next()? {
        "some" => Some(input.next()?.to_owned()),
        "none" => None,
        _ => return Err(invalid(())),
    };
    let current_payload = match input.next()? {
        "some" => Some(input.next()?.to_owned()),
        "none" => None,
        _ => return Err(invalid(())),
    };
    input.finish()?;
    Ok(StoredSnapshotBinding {
        revision,
        submission_id,
        current_payload,
    })
}
