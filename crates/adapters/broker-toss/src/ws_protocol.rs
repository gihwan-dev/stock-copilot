use domain::{
    AccountId, BrokerOrderId, Currency, KnownMarket, NumericInput, Price, Quantity, Symbol,
    validate_market_symbol,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_TOPICS_PER_CONNECTION: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WsChannel {
    TradeKr,
    TradeUs,
    OrderbookKr,
    OrderbookUs,
    PersonalOrder,
}

impl WsChannel {
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::TradeKr => "trade:kr",
            Self::TradeUs => "trade:us",
            Self::OrderbookKr => "orderbook:kr",
            Self::OrderbookUs => "orderbook:us",
            Self::PersonalOrder => "personal:order",
        }
    }

    fn parse_wire_name(raw: &str) -> Option<Self> {
        match raw {
            "trade:kr" => Some(Self::TradeKr),
            "trade:us" => Some(Self::TradeUs),
            "orderbook:kr" => Some(Self::OrderbookKr),
            "orderbook:us" => Some(Self::OrderbookUs),
            "personal:order" => Some(Self::PersonalOrder),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WsTopic {
    channel: WsChannel,
    code: String,
}

impl WsTopic {
    #[must_use]
    pub fn trade(market: KnownMarket, symbol: &Symbol) -> Self {
        let channel = match market {
            KnownMarket::Kr => WsChannel::TradeKr,
            KnownMarket::Us => WsChannel::TradeUs,
        };
        Self {
            channel,
            code: symbol.as_str().to_owned(),
        }
    }

    #[must_use]
    pub fn orderbook(market: KnownMarket, symbol: &Symbol) -> Self {
        let channel = match market {
            KnownMarket::Kr => WsChannel::OrderbookKr,
            KnownMarket::Us => WsChannel::OrderbookUs,
        };
        Self {
            channel,
            code: symbol.as_str().to_owned(),
        }
    }

    #[must_use]
    pub fn personal_order(account: &AccountId) -> Self {
        Self {
            channel: WsChannel::PersonalOrder,
            code: account.as_i64().to_string(),
        }
    }

    #[must_use]
    pub const fn channel(&self) -> WsChannel {
        self.channel
    }

    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    #[must_use]
    pub fn topic_key(&self) -> String {
        format!("{}:{}", self.channel.wire_name(), self.code)
    }

    #[must_use]
    pub fn parse_topic_key(raw: &str) -> Option<Self> {
        let (head, code) = raw.rsplit_once(':')?;
        if code.is_empty() {
            return None;
        }
        let channel = WsChannel::parse_wire_name(head)?;
        Some(Self {
            channel,
            code: code.to_owned(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsProtocolError {
    TooManyTopics,
    InvalidJson,
    UnknownFrameType,
    MissingField,
    InvalidPayload,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WsSubscriptionSet {
    topics: BTreeSet<WsTopic>,
}

impl WsSubscriptionSet {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            topics: BTreeSet::new(),
        }
    }

    pub fn insert(&mut self, topic: WsTopic) -> Result<(), WsProtocolError> {
        if !self.topics.contains(&topic) && self.topics.len() >= MAX_TOPICS_PER_CONNECTION {
            return Err(WsProtocolError::TooManyTopics);
        }
        self.topics.insert(topic);
        Ok(())
    }

    pub fn remove(&mut self, topic: &WsTopic) -> bool {
        self.topics.remove(topic)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.topics.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.topics.is_empty()
    }

    #[must_use]
    pub fn contains(&self, topic: &WsTopic) -> bool {
        self.topics.contains(topic)
    }

    pub fn apply_rejected(&mut self, rejected: &[WsRejectedTopic]) -> Vec<WsTopic> {
        let mut removed = Vec::new();
        for item in rejected {
            if let Some(topic) = WsTopic::parse_topic_key(&item.topic)
                && self.topics.remove(&topic)
            {
                removed.push(topic);
            }
        }
        removed
    }

    #[must_use]
    pub fn subscribed_accounts(&self) -> Vec<AccountId> {
        self.topics
            .iter()
            .filter(|topic| topic.channel == WsChannel::PersonalOrder)
            .filter_map(|topic| topic.code.parse::<i64>().ok().map(AccountId::new))
            .collect()
    }

    #[must_use]
    pub fn encode_declaration(&self, declaration_id: Option<&str>) -> String {
        if self.topics.is_empty() {
            return "[]".to_owned();
        }
        let mut grouped: BTreeMap<WsChannel, Vec<&str>> = BTreeMap::new();
        for topic in &self.topics {
            grouped.entry(topic.channel).or_default().push(&topic.code);
        }
        let mut entries = Vec::with_capacity(grouped.len().saturating_add(1));
        if let Some(id_text) = declaration_id {
            entries.push(json!({ "id": id_text }));
        }
        for (channel, codes) in grouped {
            entries.push(json!({
                "type": channel.wire_name(),
                "codes": codes,
            }));
        }
        Value::Array(entries).to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsErrorCode {
    WrongFormat,
    NoType,
    InvalidType,
    NoCodes,
    TooManyTopics,
    TooMany,
    RateLimitExceeded,
    InternalError,
    AuthRevoked,
    ServerShutdown,
    Unknown(String),
}

impl WsErrorCode {
    fn from_wire(raw: &str) -> Self {
        match raw {
            "wrong-format" => Self::WrongFormat,
            "no-type" => Self::NoType,
            "invalid-type" => Self::InvalidType,
            "no-codes" => Self::NoCodes,
            "too-many-topics" => Self::TooManyTopics,
            "too-many" => Self::TooMany,
            "rate-limit-exceeded" => Self::RateLimitExceeded,
            "internal-error" => Self::InternalError,
            "auth-revoked" => Self::AuthRevoked,
            "server-shutdown" => Self::ServerShutdown,
            other => Self::Unknown(other.to_owned()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsRejectedCode {
    StockNotFound,
    SymbolMarketMismatch,
    AccountNotFound,
    Unknown(String),
}

impl WsRejectedCode {
    fn from_wire(raw: &str) -> Self {
        match raw {
            "stock-not-found" => Self::StockNotFound,
            "symbol-market-mismatch" => Self::SymbolMarketMismatch,
            "account-not-found" => Self::AccountNotFound,
            other => Self::Unknown(other.to_owned()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsRejectedTopic {
    pub topic: String,
    pub code: WsRejectedCode,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsOrderEventKind {
    Pending,
    PartialFill,
    Fill,
    Canceling,
    Canceled,
    Replacing,
    Replaced,
    Rejected,
    CancelRejected,
    ReplaceRejected,
    Unknown(String),
}

impl WsOrderEventKind {
    fn from_wire(raw: &str) -> Self {
        match raw {
            "PENDING" => Self::Pending,
            "PARTIAL_FILL" => Self::PartialFill,
            "FILL" => Self::Fill,
            "CANCELING" => Self::Canceling,
            "CANCELED" => Self::Canceled,
            "REPLACING" => Self::Replacing,
            "REPLACED" => Self::Replaced,
            "REJECTED" => Self::Rejected,
            "CANCEL_REJECTED" => Self::CancelRejected,
            "REPLACE_REJECTED" => Self::ReplaceRejected,
            other => Self::Unknown(other.to_owned()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsTradeTick {
    pub market: KnownMarket,
    pub symbol: Symbol,
    pub price: Price,
    pub volume: Option<Quantity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsOrderbookTick {
    pub market: KnownMarket,
    pub symbol: Symbol,
    pub best_bid: Option<Price>,
    pub best_ask: Option<Price>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WsOrderNotice {
    pub account_id: AccountId,
    pub event_kind: WsOrderEventKind,
    pub order_id: Option<BrokerOrderId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WsServerEvent {
    SubscriptionsAck {
        id: Option<String>,
        subscribed: Vec<String>,
        rejected: Vec<WsRejectedTopic>,
    },
    Pong,
    Error {
        id: Option<String>,
        code: WsErrorCode,
        message: Option<String>,
    },
    Trade(WsTradeTick),
    Orderbook(WsOrderbookTick),
    PersonalOrder(WsOrderNotice),
    UnknownTopic {
        topic: String,
    },
}

fn parse_price(raw: &str, currency: Currency) -> Result<Price, WsProtocolError> {
    let numeric = NumericInput::try_new(raw).map_err(|_error| WsProtocolError::InvalidPayload)?;
    Price::parse(&numeric, currency).map_err(|_error| WsProtocolError::InvalidPayload)
}

fn parse_quantity(raw: &str) -> Result<Quantity, WsProtocolError> {
    let numeric = NumericInput::try_new(raw).map_err(|_error| WsProtocolError::InvalidPayload)?;
    Quantity::parse(&numeric).map_err(|_error| WsProtocolError::InvalidPayload)
}

fn market_currency(market: KnownMarket) -> Currency {
    match market {
        KnownMarket::Kr => Currency::Krw,
        KnownMarket::Us => Currency::Usd,
    }
}

fn parse_symbol(code: &str, market: KnownMarket) -> Result<Symbol, WsProtocolError> {
    let symbol = Symbol::try_new(code).map_err(|_error| WsProtocolError::InvalidPayload)?;
    validate_market_symbol(&symbol, market).map_err(|_error| WsProtocolError::InvalidPayload)?;
    Ok(symbol)
}

fn decode_trade(
    market: KnownMarket,
    code: &str,
    data: &Value,
) -> Result<WsServerEvent, WsProtocolError> {
    let symbol = parse_symbol(code, market)?;
    let currency = market_currency(market);
    let raw_price = data
        .get("price")
        .or_else(|| data.get("tradePrice"))
        .and_then(Value::as_str)
        .ok_or(WsProtocolError::MissingField)?;
    let price = parse_price(raw_price, currency)?;
    let volume = data
        .get("volume")
        .or_else(|| data.get("quantity"))
        .and_then(Value::as_str)
        .map(parse_quantity)
        .transpose()?;
    Ok(WsServerEvent::Trade(WsTradeTick {
        market,
        symbol,
        price,
        volume,
    }))
}

fn first_level_price(
    data: &Value,
    list_key: &str,
    fallback_key: &str,
    currency: Currency,
) -> Result<Option<Price>, WsProtocolError> {
    let from_list = data
        .get(list_key)
        .and_then(Value::as_array)
        .and_then(|levels| levels.first())
        .and_then(|level| level.get("price"))
        .and_then(Value::as_str);
    let raw = from_list.or_else(|| data.get(fallback_key).and_then(Value::as_str));
    raw.map(|text| parse_price(text, currency)).transpose()
}

fn decode_orderbook(
    market: KnownMarket,
    code: &str,
    data: &Value,
) -> Result<WsServerEvent, WsProtocolError> {
    let symbol = parse_symbol(code, market)?;
    let currency = market_currency(market);
    let best_bid = first_level_price(data, "bids", "bidPrice", currency)?;
    let best_ask = first_level_price(data, "asks", "askPrice", currency)?;
    Ok(WsServerEvent::Orderbook(WsOrderbookTick {
        market,
        symbol,
        best_bid,
        best_ask,
    }))
}

fn decode_personal_order(code: &str, data: &Value) -> Result<WsServerEvent, WsProtocolError> {
    let account_seq = code
        .parse::<i64>()
        .map_err(|_error| WsProtocolError::InvalidPayload)?;
    let raw_kind = data
        .get("eventType")
        .or_else(|| data.get("type"))
        .or_else(|| data.get("event"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let raw_order_id = data
        .get("order")
        .and_then(|order| order.get("id").or_else(|| order.get("orderId")))
        .or_else(|| data.get("orderId"))
        .and_then(Value::as_str);
    let order_id = raw_order_id.and_then(|value| BrokerOrderId::try_new(value).ok());
    Ok(WsServerEvent::PersonalOrder(WsOrderNotice {
        account_id: AccountId::new(account_seq),
        event_kind: WsOrderEventKind::from_wire(raw_kind),
        order_id,
    }))
}

fn decode_subscriptions_ack(frame: &Value) -> Result<WsServerEvent, WsProtocolError> {
    let id = frame.get("id").and_then(Value::as_str).map(str::to_owned);
    let subscribed = frame
        .get("subscribed")
        .and_then(Value::as_array)
        .ok_or(WsProtocolError::MissingField)?
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    let rejected = frame
        .get("rejected")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |items| {
            items
                .iter()
                .filter_map(|item| {
                    let topic = item.get("topic")?.as_str()?.to_owned();
                    let code = WsRejectedCode::from_wire(item.get("code")?.as_str()?);
                    let message = item
                        .get("message")
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                    Some(WsRejectedTopic {
                        topic,
                        code,
                        message,
                    })
                })
                .collect()
        });
    Ok(WsServerEvent::SubscriptionsAck {
        id,
        subscribed,
        rejected,
    })
}

fn decode_error(frame: &Value) -> Result<WsServerEvent, WsProtocolError> {
    let id = frame.get("id").and_then(Value::as_str).map(str::to_owned);
    let body = frame.get("error").ok_or(WsProtocolError::MissingField)?;
    let raw_code = body
        .get("code")
        .and_then(Value::as_str)
        .ok_or(WsProtocolError::MissingField)?;
    let message = body
        .get("message")
        .and_then(Value::as_str)
        .map(str::to_owned);
    Ok(WsServerEvent::Error {
        id,
        code: WsErrorCode::from_wire(raw_code),
        message,
    })
}

pub fn decode_server_text(text: &str) -> Result<WsServerEvent, WsProtocolError> {
    let frame: Value = serde_json::from_str(text).map_err(|_error| WsProtocolError::InvalidJson)?;
    let kind = frame
        .get("type")
        .and_then(Value::as_str)
        .ok_or(WsProtocolError::MissingField)?;
    match kind {
        "pong" => Ok(WsServerEvent::Pong),
        "subscriptions" => decode_subscriptions_ack(&frame),
        "error" => decode_error(&frame),
        "message" => {
            let topic_str = frame
                .get("topic")
                .and_then(Value::as_str)
                .ok_or(WsProtocolError::MissingField)?;
            let data = frame.get("data").ok_or(WsProtocolError::MissingField)?;
            let Some(topic) = WsTopic::parse_topic_key(topic_str) else {
                return Ok(WsServerEvent::UnknownTopic {
                    topic: topic_str.to_owned(),
                });
            };
            match topic.channel {
                WsChannel::TradeKr => decode_trade(KnownMarket::Kr, &topic.code, data),
                WsChannel::TradeUs => decode_trade(KnownMarket::Us, &topic.code, data),
                WsChannel::OrderbookKr => decode_orderbook(KnownMarket::Kr, &topic.code, data),
                WsChannel::OrderbookUs => decode_orderbook(KnownMarket::Us, &topic.code, data),
                WsChannel::PersonalOrder => decode_personal_order(&topic.code, data),
            }
        }
        _ => Err(WsProtocolError::UnknownFrameType),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_TOPICS_PER_CONNECTION, WsErrorCode, WsOrderEventKind, WsProtocolError, WsRejectedCode,
        WsRejectedTopic, WsServerEvent, WsSubscriptionSet, WsTopic, decode_server_text,
    };
    use domain::{AccountId, KnownMarket, Symbol};

    #[test]
    fn subscription_set_enforces_limit_serializes_full_replace_and_prunes_rejected() {
        let mut set = WsSubscriptionSet::new();
        for index in 0_usize..MAX_TOPICS_PER_CONNECTION {
            let symbol = Symbol::try_new(&format!("{index:06}")).expect("symbol");
            set.insert(WsTopic::trade(KnownMarket::Kr, &symbol))
                .expect("within limit");
        }
        let extra = Symbol::try_new("999999").expect("symbol");
        assert_eq!(
            set.insert(WsTopic::trade(KnownMarket::Kr, &extra)),
            Err(WsProtocolError::TooManyTopics)
        );
        let removed = set.apply_rejected(&[WsRejectedTopic {
            topic: "trade:kr:000000".to_owned(),
            code: WsRejectedCode::StockNotFound,
            message: None,
        }]);
        assert_eq!(removed.len(), 1);
        assert_eq!(set.len(), 99);
        assert_eq!(
            WsSubscriptionSet::new().encode_declaration(Some("id-1")),
            "[]"
        );
    }

    #[test]
    fn decoder_handles_quotes_unknown_order_events_and_error_codes() {
        let trade = decode_server_text(
            r#"{"type":"message","topic":"trade:kr:005930","data":{"price":"72500","volume":"10"}}"#,
        )
        .expect("trade");
        let WsServerEvent::Trade(tick) = trade else {
            panic!("unexpected event: {trade:?}");
        };
        assert_eq!(tick.market, KnownMarket::Kr);
        assert_eq!(tick.symbol.as_str(), "005930");

        let order = decode_server_text(
            r#"{"type":"message","topic":"personal:order:3","data":{"eventType":"FUTURE_KIND","order":{"id":"ord-7"}}}"#,
        )
        .expect("order");
        let WsServerEvent::PersonalOrder(notice) = order else {
            panic!("unexpected event: {order:?}");
        };
        assert_eq!(notice.account_id, AccountId::new(3));
        assert_eq!(
            notice.event_kind,
            WsOrderEventKind::Unknown("FUTURE_KIND".to_owned())
        );
        assert_eq!(notice.order_id.expect("id").as_str(), "ord-7");

        let revoked = decode_server_text(
            r#"{"type":"error","error":{"code":"auth-revoked","message":"expired"}}"#,
        )
        .expect("error");
        let WsServerEvent::Error { code, .. } = revoked else {
            panic!("unexpected event: {revoked:?}");
        };
        assert_eq!(code, WsErrorCode::AuthRevoked);
    }
}
