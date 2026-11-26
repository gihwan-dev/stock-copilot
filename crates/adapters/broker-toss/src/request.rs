use crate::TossHttpError;
use application::SecretValue;
use reqwest::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteId(String);
impl RouteId {
    pub fn new(value: &str) -> Result<Self, TossHttpError> {
        if value.is_empty()
            || value.len() > 128
            || value == "."
            || value == ".."
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
        {
            return Err(TossHttpError::InvalidEndpoint);
        }
        Ok(Self(value.to_owned()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadRoute {
    Accounts,
    Holdings,
    Transactions,
    Quotes,
    Candles,
    BuyingPower,
    SellableQuantity,
    Orders,
    FxRate,
    Orderbook,
    PriceLimits,
    CommissionRate,
    Instruments,
    Stocks,
    Rankings,
    InvestorRankings,
    IndicatorQuotes,
    Sectors,
    SectorRankings,
    Order(RouteId),
    StockWarnings(RouteId),
    StockInvestorFlow(RouteId),
    ProgramTrades(RouteId),
    ShortSelling(RouteId),
    CreditTrades(RouteId),
    SecuritiesLending(RouteId),
    IndicatorCandles(RouteId),
    IndicatorInvestorFlow(RouteId),
    Sector(RouteId),
    SectorStocks(RouteId),
    SectorEtfs(RouteId),
    CalendarKr,
    CalendarUs,
}
impl ReadRoute {
    const fn template(&self) -> &'static str {
        match self {
            Self::Accounts => "/api/v1/accounts",
            Self::Holdings => "/api/v1/holdings",
            Self::Transactions => "/api/v1/transactions",
            Self::Quotes => "/api/v1/prices",
            Self::Candles => "/api/v1/candles",
            Self::BuyingPower => "/api/v1/buying-power",
            Self::SellableQuantity => "/api/v1/sellable-quantity",
            Self::Orders => "/api/v1/orders",
            Self::FxRate => "/api/v1/exchange-rate",
            Self::Orderbook => "/api/v1/orderbook",
            Self::PriceLimits => "/api/v1/price-limits",
            Self::CommissionRate => "/api/v1/commissions",
            Self::Instruments => "/api/v1/stocks/all",
            Self::Stocks => "/api/v1/stocks",
            Self::Rankings => "/api/v1/rankings",
            Self::InvestorRankings => "/api/v1/rankings/investor-trading",
            Self::IndicatorQuotes => "/api/v1/market-indicators/prices",
            Self::Sectors => "/api/v1/sectors",
            Self::SectorRankings => "/api/v1/sectors/rankings",
            Self::Order(_) => "/api/v1/orders/{orderId}",
            Self::StockWarnings(_) => "/api/v1/stocks/{symbol}/warnings",
            Self::StockInvestorFlow(_) => "/api/v1/stocks/{symbol}/investor-trading",
            Self::ProgramTrades(_) => "/api/v1/stocks/{symbol}/program-trades",
            Self::ShortSelling(_) => "/api/v1/stocks/{symbol}/short-selling",
            Self::CreditTrades(_) => "/api/v1/stocks/{symbol}/credit-trades",
            Self::SecuritiesLending(_) => "/api/v1/stocks/{symbol}/securities-lending",
            Self::IndicatorCandles(_) => "/api/v1/market-indicators/{symbol}/candles",
            Self::IndicatorInvestorFlow(_) => "/api/v1/market-indicators/{symbol}/investor-trading",
            Self::Sector(_) => "/api/v1/sectors/{sectorId}",
            Self::SectorStocks(_) => "/api/v1/sectors/{sectorId}/stocks",
            Self::SectorEtfs(_) => "/api/v1/sectors/{sectorId}/etfs",
            Self::CalendarKr => "/api/v1/market-calendar/KR",
            Self::CalendarUs => "/api/v1/market-calendar/US",
        }
    }
    fn path(&self) -> String {
        match self {
            Self::Accounts => "/api/v1/accounts".to_owned(),
            Self::Holdings => "/api/v1/holdings".to_owned(),
            Self::Transactions => "/api/v1/transactions".to_owned(),
            Self::Quotes => "/api/v1/prices".to_owned(),
            Self::Candles => "/api/v1/candles".to_owned(),
            Self::BuyingPower => "/api/v1/buying-power".to_owned(),
            Self::SellableQuantity => "/api/v1/sellable-quantity".to_owned(),
            Self::Orders => "/api/v1/orders".to_owned(),
            Self::FxRate => "/api/v1/exchange-rate".to_owned(),
            Self::Orderbook => "/api/v1/orderbook".to_owned(),
            Self::PriceLimits => "/api/v1/price-limits".to_owned(),
            Self::CommissionRate => "/api/v1/commissions".to_owned(),
            Self::Instruments => "/api/v1/stocks/all".to_owned(),
            Self::Stocks => "/api/v1/stocks".to_owned(),
            Self::Rankings => "/api/v1/rankings".to_owned(),
            Self::InvestorRankings => "/api/v1/rankings/investor-trading".to_owned(),
            Self::IndicatorQuotes => "/api/v1/market-indicators/prices".to_owned(),
            Self::Sectors => "/api/v1/sectors".to_owned(),
            Self::SectorRankings => "/api/v1/sectors/rankings".to_owned(),
            Self::Order(id) => format!("/api/v1/orders/{}", id.0),
            Self::StockWarnings(id) => format!("/api/v1/stocks/{}/warnings", id.0),
            Self::StockInvestorFlow(id) => format!("/api/v1/stocks/{}/investor-trading", id.0),
            Self::ProgramTrades(id) => format!("/api/v1/stocks/{}/program-trades", id.0),
            Self::ShortSelling(id) => format!("/api/v1/stocks/{}/short-selling", id.0),
            Self::CreditTrades(id) => format!("/api/v1/stocks/{}/credit-trades", id.0),
            Self::SecuritiesLending(id) => format!("/api/v1/stocks/{}/securities-lending", id.0),
            Self::IndicatorCandles(id) => format!("/api/v1/market-indicators/{}/candles", id.0),
            Self::IndicatorInvestorFlow(id) => {
                format!("/api/v1/market-indicators/{}/investor-trading", id.0)
            }
            Self::Sector(id) => format!("/api/v1/sectors/{}", id.0),
            Self::SectorStocks(id) => format!("/api/v1/sectors/{}/stocks", id.0),
            Self::SectorEtfs(id) => format!("/api/v1/sectors/{}/etfs", id.0),
            Self::CalendarKr => "/api/v1/market-calendar/KR".to_owned(),
            Self::CalendarUs => "/api/v1/market-calendar/US".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryKey {
    Symbol,
    Symbols,
    Market,
    Count,
    Before,
    Adjusted,
    Interval,
    From,
    To,
    Until,
    Cursor,
    Limit,
    Status,
    Type,
    Duration,
    ExcludeInvestmentCaution,
    MarketCountry,
    DateTime,
    BaseCurrency,
    QuoteCurrency,
    TransactionTypes,
    Currency,
    SecurityType,
    CommonShare,
    Date,
}
impl QueryKey {
    const fn name(self) -> &'static str {
        match self {
            Self::Symbol => "symbol",
            Self::Symbols => "symbols",
            Self::Market => "market",
            Self::Count => "count",
            Self::Before => "before",
            Self::Adjusted => "adjusted",
            Self::Interval => "interval",
            Self::From => "from",
            Self::To => "to",
            Self::Until => "until",
            Self::Cursor => "cursor",
            Self::Limit => "limit",
            Self::Status => "status",
            Self::Type => "type",
            Self::Duration => "duration",
            Self::ExcludeInvestmentCaution => "excludeInvestmentCaution",
            Self::MarketCountry => "marketCountry",
            Self::DateTime => "dateTime",
            Self::BaseCurrency => "baseCurrency",
            Self::QuoteCurrency => "quoteCurrency",
            Self::TransactionTypes => "transactionTypes",
            Self::Currency => "currency",
            Self::SecurityType => "securityType",
            Self::CommonShare => "commonShare",
            Self::Date => "date",
        }
    }
}

pub struct ReadRequest {
    route: ReadRoute,
    query: Vec<(QueryKey, String)>,
    account: Option<SecretValue>,
}
impl std::fmt::Debug for ReadRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ReadRequest(<redacted>)")
    }
}
impl ReadRequest {
    pub(crate) fn group(&self) -> crate::rate_limiter::Group {
        crate::rate_limiter::Group::route(&self.route)
    }
    #[must_use]
    pub const fn new(route: ReadRoute) -> Self {
        Self {
            route,
            query: Vec::new(),
            account: None,
        }
    }
    #[must_use]
    pub fn with_query(mut self, key: QueryKey, value: String) -> Self {
        self.query.push((key, value));
        self
    }
    #[must_use]
    pub fn with_account(mut self, account: SecretValue) -> Self {
        self.account = Some(account);
        self
    }
    pub(crate) const fn path_template(&self) -> &'static str {
        self.route.template()
    }
    pub(crate) const fn account(&self) -> Option<&SecretValue> {
        self.account.as_ref()
    }
    pub(crate) fn url(&self, origin: &Url) -> Url {
        let mut url = origin.clone();
        url.set_path(&self.route.path());
        if !self.query.is_empty() {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in &self.query {
                pairs.append_pair(key.name(), value);
            }
        }
        url
    }
}

#[cfg(test)]
mod tests {
    use super::{QueryKey, ReadRequest, ReadRoute, RouteId};
    use crate::TossHttpError;
    use reqwest::Url;
    fn static_routes() -> Vec<(ReadRoute, &'static str)> {
        vec![
            (ReadRoute::Accounts, "/api/v1/accounts"),
            (ReadRoute::Holdings, "/api/v1/holdings"),
            (ReadRoute::Transactions, "/api/v1/transactions"),
            (ReadRoute::Quotes, "/api/v1/prices"),
            (ReadRoute::Candles, "/api/v1/candles"),
            (ReadRoute::BuyingPower, "/api/v1/buying-power"),
            (ReadRoute::SellableQuantity, "/api/v1/sellable-quantity"),
            (ReadRoute::Orders, "/api/v1/orders"),
            (ReadRoute::FxRate, "/api/v1/exchange-rate"),
            (ReadRoute::Orderbook, "/api/v1/orderbook"),
            (ReadRoute::PriceLimits, "/api/v1/price-limits"),
            (ReadRoute::CommissionRate, "/api/v1/commissions"),
            (ReadRoute::Instruments, "/api/v1/stocks/all"),
            (ReadRoute::Stocks, "/api/v1/stocks"),
            (ReadRoute::Rankings, "/api/v1/rankings"),
            (
                ReadRoute::InvestorRankings,
                "/api/v1/rankings/investor-trading",
            ),
            (
                ReadRoute::IndicatorQuotes,
                "/api/v1/market-indicators/prices",
            ),
            (ReadRoute::Sectors, "/api/v1/sectors"),
            (ReadRoute::SectorRankings, "/api/v1/sectors/rankings"),
            (ReadRoute::CalendarKr, "/api/v1/market-calendar/KR"),
            (ReadRoute::CalendarUs, "/api/v1/market-calendar/US"),
        ]
    }
    fn dynamic_routes() -> Vec<(ReadRoute, &'static str)> {
        vec![
            (
                ReadRoute::Order(RouteId::new("Value-71").expect("id")),
                "/api/v1/orders/Value-71",
            ),
            (
                ReadRoute::StockWarnings(RouteId::new("Value-71").expect("id")),
                "/api/v1/stocks/Value-71/warnings",
            ),
            (
                ReadRoute::StockInvestorFlow(RouteId::new("Value-71").expect("id")),
                "/api/v1/stocks/Value-71/investor-trading",
            ),
            (
                ReadRoute::ProgramTrades(RouteId::new("Value-71").expect("id")),
                "/api/v1/stocks/Value-71/program-trades",
            ),
            (
                ReadRoute::ShortSelling(RouteId::new("Value-71").expect("id")),
                "/api/v1/stocks/Value-71/short-selling",
            ),
            (
                ReadRoute::CreditTrades(RouteId::new("Value-71").expect("id")),
                "/api/v1/stocks/Value-71/credit-trades",
            ),
            (
                ReadRoute::SecuritiesLending(RouteId::new("Value-71").expect("id")),
                "/api/v1/stocks/Value-71/securities-lending",
            ),
            (
                ReadRoute::IndicatorCandles(RouteId::new("Value-71").expect("id")),
                "/api/v1/market-indicators/Value-71/candles",
            ),
            (
                ReadRoute::IndicatorInvestorFlow(RouteId::new("Value-71").expect("id")),
                "/api/v1/market-indicators/Value-71/investor-trading",
            ),
            (
                ReadRoute::Sector(RouteId::new("Value-71").expect("id")),
                "/api/v1/sectors/Value-71",
            ),
            (
                ReadRoute::SectorStocks(RouteId::new("Value-71").expect("id")),
                "/api/v1/sectors/Value-71/stocks",
            ),
            (
                ReadRoute::SectorEtfs(RouteId::new("Value-71").expect("id")),
                "/api/v1/sectors/Value-71/etfs",
            ),
        ]
    }
    #[test]
    fn closed_routes_match_the_openapi_get_paths() {
        let origin = Url::parse("http://127.0.0.1:1234").expect("local");
        let spec: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string("../../../docs/toss-openapi-1.2.28.json").expect("spec"),
        )
        .expect("JSON");
        for (route, path) in static_routes().into_iter().chain(dynamic_routes()) {
            let request = ReadRequest::new(route);
            assert_eq!(request.url(&origin).path(), path);
            let template = path.replace("Value-71", "{symbol}");
            let found = spec["paths"]
                .as_object()
                .expect("paths")
                .iter()
                .any(|(key, value)| {
                    value.get("get").is_some()
                        && (key == &template
                            || key
                                .replace("{orderId}", "{symbol}")
                                .replace("{sectorId}", "{symbol}")
                                == template)
                });
            assert!(found, "{path}");
        }
    }
    #[test]
    fn dynamic_identifiers_cannot_escape_their_single_path_segment() {
        for value in ["", ".", "..", "a/b", "a?b", "a#b", "%2F", "a\\b", "한글"] {
            assert_eq!(
                RouteId::new(value).expect_err("invalid"),
                TossHttpError::InvalidEndpoint
            );
        }
        assert_eq!(
            RouteId::new(&"a".repeat(128)).expect("boundary").0.len(),
            128
        );
        assert_eq!(
            RouteId::new(&"a".repeat(129)).expect_err("too long"),
            TossHttpError::InvalidEndpoint
        );
        assert_eq!(RouteId::new("a_7.B-C").expect("id").0, "a_7.B-C");
    }
    #[test]
    fn all_query_keys_are_closed_and_exact() {
        assert_eq!(QueryKey::Adjusted.name(), "adjusted");
        assert_eq!(QueryKey::BaseCurrency.name(), "baseCurrency");
        assert_eq!(QueryKey::Before.name(), "before");
        assert_eq!(QueryKey::CommonShare.name(), "commonShare");
        assert_eq!(QueryKey::Count.name(), "count");
        assert_eq!(QueryKey::Currency.name(), "currency");
        assert_eq!(QueryKey::Cursor.name(), "cursor");
        assert_eq!(QueryKey::Date.name(), "date");
        assert_eq!(QueryKey::DateTime.name(), "dateTime");
        assert_eq!(QueryKey::Duration.name(), "duration");
        assert_eq!(
            QueryKey::ExcludeInvestmentCaution.name(),
            "excludeInvestmentCaution"
        );
        assert_eq!(QueryKey::From.name(), "from");
        assert_eq!(QueryKey::Interval.name(), "interval");
        assert_eq!(QueryKey::Limit.name(), "limit");
        assert_eq!(QueryKey::Market.name(), "market");
        assert_eq!(QueryKey::MarketCountry.name(), "marketCountry");
        assert_eq!(QueryKey::QuoteCurrency.name(), "quoteCurrency");
        assert_eq!(QueryKey::SecurityType.name(), "securityType");
        assert_eq!(QueryKey::Status.name(), "status");
        assert_eq!(QueryKey::Symbol.name(), "symbol");
        assert_eq!(QueryKey::Symbols.name(), "symbols");
        assert_eq!(QueryKey::To.name(), "to");
        assert_eq!(QueryKey::TransactionTypes.name(), "transactionTypes");
        assert_eq!(QueryKey::Type.name(), "type");
        assert_eq!(QueryKey::Until.name(), "until");
    }
}
