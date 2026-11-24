use crate::{TossDate, TossTimestamp};
use domain::{
    AccountId, BrokerOrderId, BrokerOrderMoney, BrokerOrderPrice, BrokerOrderRow, BrokerOrderSize,
    BrokerOrderStatus, BrokerOrderTime, BrokerOrderType, BrokerTimeInForce, Currency, KnownMarket,
    MasterCode, MasterInstrumentFacts, Money, NumericInput, OrderSide, Price, Quantity,
    ResolvedOrderInstrument, Symbol, Timestamp, validate_market_symbol,
};

pub(crate) enum RowFailure {
    Schema,
    MissingMaster,
    Unidentifiable,
}

pub(crate) struct ParsedOrder {
    pub(crate) order_id: BrokerOrderId,
    pub(crate) instrument: ResolvedOrderInstrument,
    pub(crate) side: OrderSide,
    pub(crate) status: BrokerOrderStatus,
    pub(crate) size: BrokerOrderSize,
    pub(crate) filled_quantity: Quantity,
    pub(crate) average_filled_price: Option<Price>,
    pub(crate) filled_amount: BrokerOrderMoney,
    pub(crate) filled_at: Option<Timestamp>,
    pub(crate) ordered_at: Timestamp,
    pub(crate) canceled_at: BrokerOrderTime,
    pub(crate) price: BrokerOrderPrice,
    pub(crate) order_type: BrokerOrderType,
    pub(crate) time_in_force: BrokerTimeInForce,
}

impl ParsedOrder {
    fn probe_row(&self) -> BrokerOrderRow {
        BrokerOrderRow {
            account_id: AccountId::new(0),
            order_id: self.order_id.clone(),
            instrument: self.instrument.clone(),
            side: self.side,
            status: self.status.clone(),
            size: self.size,
            filled_quantity: self.filled_quantity,
            average_filled_price: self.average_filled_price,
            filled_amount: self.filled_amount,
            filled_at: self.filled_at,
            ordered_at: self.ordered_at,
            canceled_at: self.canceled_at,
            price: self.price,
            order_type: self.order_type.clone(),
            time_in_force: self.time_in_force.clone(),
            observed_at: self.ordered_at,
        }
    }

    pub(crate) fn into_row(self, account: &AccountId, observed: Timestamp) -> BrokerOrderRow {
        BrokerOrderRow {
            account_id: account.clone(),
            order_id: self.order_id,
            instrument: self.instrument,
            side: self.side,
            status: self.status,
            size: self.size,
            filled_quantity: self.filled_quantity,
            average_filled_price: self.average_filled_price,
            filled_amount: self.filled_amount,
            filled_at: self.filled_at,
            ordered_at: self.ordered_at,
            canceled_at: self.canceled_at,
            price: self.price,
            order_type: self.order_type,
            time_in_force: self.time_in_force,
            observed_at: observed,
        }
    }
}

fn numeric(raw: &str) -> Result<NumericInput, RowFailure> {
    if raw.len() > 30 {
        return Err(RowFailure::Unidentifiable);
    }
    NumericInput::try_new(raw).map_err(|_error| RowFailure::Unidentifiable)
}

fn money(raw: &str, currency: Currency) -> Result<Money, RowFailure> {
    Money::parse(&numeric(raw)?, currency).map_err(|_error| RowFailure::Unidentifiable)
}

fn price(raw: &str, currency: Currency) -> Result<Price, RowFailure> {
    Price::parse(&numeric(raw)?, currency).map_err(|_error| RowFailure::Unidentifiable)
}

fn quantity(raw: &str) -> Result<Quantity, RowFailure> {
    Quantity::parse(&numeric(raw)?).map_err(|_error| RowFailure::Unidentifiable)
}

fn timestamp(raw: &str) -> Result<Timestamp, RowFailure> {
    TossTimestamp::try_from(raw)
        .map(TossTimestamp::into_domain)
        .map_err(|_error| RowFailure::Unidentifiable)
}

fn resolve_instrument(
    master: &MasterInstrumentFacts,
    symbol: &Symbol,
    market: KnownMarket,
    currency: Currency,
    side: OrderSide,
) -> Result<ResolvedOrderInstrument, RowFailure> {
    let identified = master
        .instruments()
        .iter()
        .find(|row| {
            row.row().symbol == *symbol
                && row.row().currency == currency
                && row
                    .quote_instrument()
                    .is_ok_and(|instrument| *instrument.market() == market)
        })
        .ok_or(RowFailure::MissingMaster)?;
    identified
        .order_gate_facts()
        .and_then(|gate| gate.instrument_for(side))
        .map_err(|_error| RowFailure::Unidentifiable)
}

pub(crate) fn parse_raw(
    raw: &str,
    master: &MasterInstrumentFacts,
) -> Result<ParsedOrder, RowFailure> {
    let decoded: broker_toss_schema::types::Order =
        serde_json::from_str(raw).map_err(|_error| RowFailure::Schema)?;
    parse_decoded(&decoded, master)
}

struct ParsedOrderHeader {
    order_id: BrokerOrderId,
    market: KnownMarket,
    currency: Currency,
    symbol: Symbol,
    side: OrderSide,
    order_type: BrokerOrderType,
    time_in_force: BrokerTimeInForce,
    status: BrokerOrderStatus,
}

fn parse_header(
    decoded: &broker_toss_schema::types::Order,
) -> Result<ParsedOrderHeader, RowFailure> {
    let order_id =
        BrokerOrderId::try_new(&decoded.id).map_err(|_error| RowFailure::Unidentifiable)?;
    let (market, currency) = match decoded.currency {
        broker_toss_schema::types::Currency::Krw => (KnownMarket::Kr, Currency::Krw),
        broker_toss_schema::types::Currency::Usd => (KnownMarket::Us, Currency::Usd),
        broker_toss_schema::types::Currency::Unknown(_) => return Err(RowFailure::Unidentifiable),
    };
    let symbol = Symbol::try_new(&decoded.symbol).map_err(|_error| RowFailure::Unidentifiable)?;
    validate_market_symbol(&symbol, market).map_err(|_error| RowFailure::Unidentifiable)?;
    let side = match decoded.side {
        broker_toss_schema::types::OrderSide::Buy => OrderSide::Buy,
        broker_toss_schema::types::OrderSide::Sell => OrderSide::Sell,
        broker_toss_schema::types::OrderSide::Unknown(_) => return Err(RowFailure::Unidentifiable),
    };
    let order_type = BrokerOrderType::from_code(
        MasterCode::try_new(&decoded.kind.to_string())
            .map_err(|_error| RowFailure::Unidentifiable)?,
    );
    let time_in_force = BrokerTimeInForce::from_code(
        MasterCode::try_new(&decoded.time_in_force.to_string())
            .map_err(|_error| RowFailure::Unidentifiable)?,
    );
    let status = BrokerOrderStatus::from_code(
        MasterCode::try_new(&decoded.status.to_string())
            .map_err(|_error| RowFailure::Unidentifiable)?,
    );
    Ok(ParsedOrderHeader {
        order_id,
        market,
        currency,
        symbol,
        side,
        order_type,
        time_in_force,
        status,
    })
}

struct ParsedExecution {
    filled_quantity: Quantity,
    average_filled_price: Option<Price>,
    filled_amount: BrokerOrderMoney,
    filled_at: Option<Timestamp>,
}

fn parse_execution(
    execution: &broker_toss_schema::types::OrderExecution,
    currency: Currency,
    ordered_at: Timestamp,
) -> Result<ParsedExecution, RowFailure> {
    let filled_quantity = quantity(&execution.filled_quantity)?;
    let average_filled_price = execution
        .average_filled_price
        .as_deref()
        .map(|raw| price(raw, currency))
        .transpose()?;
    let filled_amount = match execution.filled_amount.as_deref() {
        None => BrokerOrderMoney::Absent,
        Some(raw) => BrokerOrderMoney::Known(money(raw, currency)?),
    };
    if let Some(raw_commission) = execution.commission.as_deref() {
        let _commission = money(raw_commission, currency)?;
    }
    if let Some(raw_tax) = execution.tax.as_deref() {
        let _tax = money(raw_tax, currency)?;
    }
    let filled_at = execution.filled_at.as_deref().map(timestamp).transpose()?;
    if filled_at.is_some_and(|at| at < ordered_at) {
        return Err(RowFailure::Unidentifiable);
    }
    if let Some(raw_settlement) = execution.settlement_date.as_deref() {
        let _settlement =
            TossDate::try_from(raw_settlement).map_err(|_error| RowFailure::Unidentifiable)?;
    }
    if filled_quantity.as_decimal().is_zero()
        && (filled_at.is_some()
            || match filled_amount {
                BrokerOrderMoney::Known(amount) => !amount.amount().as_decimal().is_zero(),
                BrokerOrderMoney::Absent => false,
            })
    {
        return Err(RowFailure::Unidentifiable);
    }
    Ok(ParsedExecution {
        filled_quantity,
        average_filled_price,
        filled_amount,
        filled_at,
    })
}

pub(crate) fn parse_decoded(
    decoded: &broker_toss_schema::types::Order,
    master: &MasterInstrumentFacts,
) -> Result<ParsedOrder, RowFailure> {
    let header = parse_header(decoded)?;
    let order_price = match decoded.price.as_deref() {
        None => BrokerOrderPrice::Absent,
        Some(raw) => BrokerOrderPrice::Known(price(raw, header.currency)?),
    };
    let order_quantity = quantity(&decoded.quantity)?;
    let order_size = match decoded.amount.as_deref() {
        None => BrokerOrderSize::Quantity(order_quantity),
        Some(raw) => BrokerOrderSize::Amount {
            reported_quantity: order_quantity,
            order_amount: money(raw, header.currency)?,
        },
    };
    let ordered_at = timestamp(&decoded.ordered_at)?;
    let canceled_at = match decoded.canceled_at.as_deref() {
        None => BrokerOrderTime::Absent,
        Some(raw) => {
            let at = timestamp(raw)?;
            if at < ordered_at {
                return Err(RowFailure::Unidentifiable);
            }
            BrokerOrderTime::Known(at)
        }
    };
    let execution = parse_execution(&decoded.execution, header.currency, ordered_at)?;
    let instrument = resolve_instrument(
        master,
        &header.symbol,
        header.market,
        header.currency,
        header.side,
    )?;
    let parsed = ParsedOrder {
        order_id: header.order_id,
        instrument,
        side: header.side,
        status: header.status,
        size: order_size,
        filled_quantity: execution.filled_quantity,
        average_filled_price: execution.average_filled_price,
        filled_amount: execution.filled_amount,
        filled_at: execution.filled_at,
        ordered_at,
        canceled_at,
        price: order_price,
        order_type: header.order_type,
        time_in_force: header.time_in_force,
    };
    domain::BrokerOrder::from_lookup_result(parsed.probe_row())
        .map_err(|_error| RowFailure::Unidentifiable)?;
    Ok(parsed)
}
