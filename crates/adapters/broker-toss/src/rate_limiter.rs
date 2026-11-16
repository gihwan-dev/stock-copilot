use crate::ReadRoute;
use application::Clock;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, MutexGuard};

pub trait RequestWait: Send + Sync {
    fn wait(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

#[derive(Debug)]
pub struct TokioRequestWait;
impl RequestWait for TokioRequestWait {
    fn wait(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(tokio::time::sleep(duration))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) enum Group {
    Account,
    Asset,
    Transaction,
    Stock,
    StockAll,
    StockTrend,
    MarketInfo,
    MarketData,
    Chart,
    Ranking,
    Indicator,
    IndicatorChart,
    Sector,
    SectorRanking,
    OrderHistory,
    OrderInfo,
}
impl Group {
    const ALL: [Self; 16] = [
        Self::Account,
        Self::Asset,
        Self::Transaction,
        Self::Stock,
        Self::StockAll,
        Self::StockTrend,
        Self::MarketInfo,
        Self::MarketData,
        Self::Chart,
        Self::Ranking,
        Self::Indicator,
        Self::IndicatorChart,
        Self::Sector,
        Self::SectorRanking,
        Self::OrderHistory,
        Self::OrderInfo,
    ];
    const fn limit(self) -> u32 {
        match self {
            Self::Account | Self::StockAll => 1,
            Self::MarketInfo | Self::OrderInfo => 3,
            Self::StockTrend | Self::Indicator => 10,
            Self::MarketData => 15,
            Self::Chart => 20,
            Self::Asset
            | Self::Transaction
            | Self::Stock
            | Self::Ranking
            | Self::IndicatorChart
            | Self::Sector
            | Self::SectorRanking
            | Self::OrderHistory => 5,
        }
    }
    pub(crate) fn route(route: &ReadRoute) -> Self {
        match route {
            ReadRoute::Accounts => Self::Account,
            ReadRoute::Holdings => Self::Asset,
            ReadRoute::Transactions => Self::Transaction,
            ReadRoute::Stocks | ReadRoute::StockWarnings(_) => Self::Stock,
            ReadRoute::Instruments => Self::StockAll,
            ReadRoute::StockInvestorFlow(_)
            | ReadRoute::ProgramTrades(_)
            | ReadRoute::ShortSelling(_)
            | ReadRoute::CreditTrades(_)
            | ReadRoute::SecuritiesLending(_) => Self::StockTrend,
            ReadRoute::FxRate | ReadRoute::CalendarKr | ReadRoute::CalendarUs => Self::MarketInfo,
            ReadRoute::Quotes | ReadRoute::Orderbook | ReadRoute::PriceLimits => Self::MarketData,
            ReadRoute::Candles => Self::Chart,
            ReadRoute::Rankings | ReadRoute::InvestorRankings => Self::Ranking,
            ReadRoute::IndicatorQuotes | ReadRoute::IndicatorInvestorFlow(_) => Self::Indicator,
            ReadRoute::IndicatorCandles(_) => Self::IndicatorChart,
            ReadRoute::Sectors
            | ReadRoute::Sector(_)
            | ReadRoute::SectorStocks(_)
            | ReadRoute::SectorEtfs(_) => Self::Sector,
            ReadRoute::SectorRankings => Self::SectorRanking,
            ReadRoute::Orders | ReadRoute::Order(_) => Self::OrderHistory,
            ReadRoute::BuyingPower | ReadRoute::SellableQuantity | ReadRoute::CommissionRate => {
                Self::OrderInfo
            }
        }
    }
}

pub(crate) struct RateLimiter {
    clock: Arc<dyn Clock>,
    wait: Arc<dyn RequestWait>,
    groups: [Mutex<Budget>; 16],
}
pub(crate) struct Budget {
    limit: u32,
    last: Option<i128>,
    blocked_until: i128,
}
impl Budget {
    fn interval(&self) -> i128 {
        {
            let denominator = i128::from(self.limit.max(1)).saturating_mul(3);
            5_000_000_000_i128
                .saturating_add(denominator)
                .saturating_sub(1)
                .checked_div(denominator)
                .map_or(0, |value| value.max(1))
        }
    }
    pub(crate) fn observe(&mut self, limit: Option<&str>) {
        if let Some(value) = limit
            .filter(|text| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()))
            .and_then(|text| text.parse::<u32>().ok())
            .filter(|value| *value > 0)
        {
            self.limit = value;
        }
    }
}
impl RateLimiter {
    pub(crate) fn new(clock: Arc<dyn Clock>, wait: Arc<dyn RequestWait>) -> Self {
        Self {
            clock,
            wait,
            groups: Group::ALL.map(|group| {
                Mutex::new(Budget {
                    limit: group.limit(),
                    last: None,
                    blocked_until: i128::MIN,
                })
            }),
        }
    }
    fn now(&self) -> i128 {
        let now = self.clock.now();
        i128::from(*now.utc_seconds().as_i64())
            .saturating_mul(1_000_000_000)
            .saturating_add(i128::from(*now.nanosecond().as_u32()))
    }
    pub(crate) async fn lock(&self, group: Group) -> MutexGuard<'_, Budget> {
        let [
            account,
            asset,
            transaction,
            stock,
            stock_all,
            stock_trend,
            market_info,
            market_data,
            chart,
            ranking,
            indicator,
            indicator_chart,
            sector,
            sector_ranking,
            order_history,
            order_info,
        ] = &self.groups;
        match group {
            Group::Account => account,
            Group::Asset => asset,
            Group::Transaction => transaction,
            Group::Stock => stock,
            Group::StockAll => stock_all,
            Group::StockTrend => stock_trend,
            Group::MarketInfo => market_info,
            Group::MarketData => market_data,
            Group::Chart => chart,
            Group::Ranking => ranking,
            Group::Indicator => indicator,
            Group::IndicatorChart => indicator_chart,
            Group::Sector => sector,
            Group::SectorRanking => sector_ranking,
            Group::OrderHistory => order_history,
            Group::OrderInfo => order_info,
        }
        .lock()
        .await
    }
    pub(crate) async fn admit(&self, budget: &mut Budget) {
        loop {
            let now = self.now();
            let deadline = budget.last.map_or(budget.blocked_until, |last| {
                last.saturating_add(budget.interval())
                    .max(budget.blocked_until)
            });
            if now >= deadline {
                budget.last = Some(now);
                return;
            }
            let nanos = deadline.saturating_sub(now);
            let duration = u64::try_from(nanos.min(86_400_000_000_000))
                .map_or(Duration::from_hours(24), Duration::from_nanos);
            self.wait.wait(duration).await;
        }
    }
    pub(crate) fn defer(&self, budget: &mut Budget, retry_after: Option<&str>, attempt: u32) {
        let seconds = retry_after
            .filter(|text| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()))
            .and_then(|text| text.parse::<u64>().ok())
            .map_or(1_000_000_000_i128, |value| {
                i128::from(value).saturating_mul(1_000_000_000)
            });
        let backoff = match attempt {
            0 => 1_000_000_000,
            1 => 2_000_000_000,
            _ => 4_000_000_000,
        };
        budget.blocked_until = budget
            .blocked_until
            .max(self.now().saturating_add(seconds).saturating_add(backoff));
    }
}

#[cfg(test)]
mod tests {
    use super::{Budget, Group, RateLimiter, RequestWait};
    use crate::ReadRoute;
    use application::Clock;
    use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    use std::task::{Context, Poll, Waker};
    use std::time::Duration;

    #[derive(Default)]
    struct Time {
        nanos: AtomicU64,
        waits: AtomicU64,
        duration: AtomicU64,
    }
    impl Clock for Time {
        fn now(&self) -> Timestamp {
            let nanos = self.nanos.load(Ordering::SeqCst);
            Timestamp::from_parts(
                UnixSeconds::new(
                    1_800_000_000_i64.saturating_add(
                        i64::try_from(nanos.checked_div(1_000_000_000).expect("divisor"))
                            .expect("seconds"),
                    ),
                ),
                Nanosecond::new(u32::try_from(nanos % 1_000_000_000).expect("nanos")),
                OffsetSeconds::new(0),
            )
            .expect("time")
        }
    }
    impl RequestWait for Time {
        fn wait(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            self.waits.fetch_add(1, Ordering::SeqCst);
            self.duration.store(duration.as_secs(), Ordering::SeqCst);
            Box::pin(std::future::pending())
        }
    }
    fn poll<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
        future.poll(&mut Context::from_waker(Waker::noop()))
    }
    fn admission_pending(limiter: &RateLimiter, budget: &mut Budget) -> bool {
        let mut future = Box::pin(limiter.admit(budget));
        poll(future.as_mut()).is_pending()
    }
    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
    }
    #[test]
    fn all_groups_have_documented_conservative_intervals() {
        for (group, limit, interval) in [
            (Group::Account, 1, 1_666_666_667),
            (Group::Asset, 5, 333_333_334),
            (Group::Transaction, 5, 333_333_334),
            (Group::Stock, 5, 333_333_334),
            (Group::StockAll, 1, 1_666_666_667),
            (Group::StockTrend, 10, 166_666_667),
            (Group::MarketInfo, 3, 555_555_556),
            (Group::MarketData, 15, 111_111_112),
            (Group::Chart, 20, 83_333_334),
            (Group::Ranking, 5, 333_333_334),
            (Group::Indicator, 10, 166_666_667),
            (Group::IndicatorChart, 5, 333_333_334),
            (Group::Sector, 5, 333_333_334),
            (Group::SectorRanking, 5, 333_333_334),
            (Group::OrderHistory, 5, 333_333_334),
            (Group::OrderInfo, 3, 555_555_556),
        ] {
            assert_eq!(group.limit(), limit, "{group:?}");
            assert_eq!(
                Budget {
                    limit,
                    last: None,
                    blocked_until: 0
                }
                .interval(),
                interval
            );
        }
    }
    #[test]
    fn headers_are_strict_and_zero_keeps_the_last_usable_limit() {
        let mut budget = Budget {
            limit: 15,
            last: None,
            blocked_until: 0,
        };
        budget.observe(Some("3"));
        assert_eq!(budget.limit, 3);
        for invalid in [
            None,
            Some(""),
            Some("+5"),
            Some("1.5"),
            Some("-1"),
            Some("4294967296"),
            Some(" 9"),
            Some("9,10"),
            Some("0x20"),
        ] {
            budget.observe(invalid);
            assert_eq!(budget.limit, 3);
        }
        budget.observe(Some("6"));
        assert_eq!(budget.limit, 6);
        budget.observe(Some("0"));
        assert_eq!(budget.limit, 6);
    }
    #[test]
    fn sixty_percent_before_exact_and_after_boundary_and_clock_rollback() {
        let time = Arc::new(Time::default());
        let limiter = RateLimiter::new(Arc::<Time>::clone(&time), Arc::<Time>::clone(&time));
        let mut budget = runtime().block_on(limiter.lock(Group::Asset));
        assert!(!admission_pending(&limiter, &mut budget));
        time.nanos.store(333_333_333, Ordering::SeqCst);
        assert!(admission_pending(&limiter, &mut budget));
        time.nanos.store(333_333_334, Ordering::SeqCst);
        assert!(!admission_pending(&limiter, &mut budget));
        time.nanos.store(666_666_669, Ordering::SeqCst);
        assert!(!admission_pending(&limiter, &mut budget));
        time.nanos.store(1, Ordering::SeqCst);
        let mut pending = Box::pin(limiter.admit(&mut budget));
        assert!(poll(pending.as_mut()).is_pending());
    }
    #[test]
    fn reduction_applies_to_previous_send_and_groups_are_independent() {
        let time = Arc::new(Time::default());
        let limiter = RateLimiter::new(Arc::<Time>::clone(&time), Arc::<Time>::clone(&time));
        let mut budget = runtime().block_on(limiter.lock(Group::MarketData));
        assert!(!admission_pending(&limiter, &mut budget));
        budget.observe(Some("1"));
        time.nanos.store(111_111_112, Ordering::SeqCst);
        assert!(admission_pending(&limiter, &mut budget));
        let mut other = runtime().block_on(limiter.lock(Group::Chart));
        assert!(!admission_pending(&limiter, &mut other));
        assert_eq!(other.last, Some(limiter.now()));
        time.nanos.store(1_666_666_667, Ordering::SeqCst);
        assert!(!admission_pending(&limiter, &mut budget));
        assert_eq!(budget.last, Some(limiter.now()));
    }
    #[test]
    fn retry_seconds_and_backoff_are_additive_capped_and_strict() {
        let time = Arc::new(Time::default());
        let limiter = RateLimiter::new(Arc::<Time>::clone(&time), time);
        for (attempt, seconds) in [(0, 8), (1, 9), (2, 11), (3, 11), (31, 11), (32, 11)] {
            let mut budget = Budget {
                limit: 5,
                last: None,
                blocked_until: 0,
            };
            limiter.defer(&mut budget, Some("7"), attempt);
            assert_eq!(
                budget.blocked_until,
                limiter.now() + seconds * 1_000_000_000
            );
        }
        for header in [
            None,
            Some(""),
            Some("-1"),
            Some("+2"),
            Some("1.2"),
            Some("Wed, 21 Oct 2015 07:28:00 GMT"),
        ] {
            let mut budget = Budget {
                limit: 5,
                last: None,
                blocked_until: 0,
            };
            limiter.defer(&mut budget, header, 0);
            assert_eq!(budget.blocked_until, limiter.now() + 2_000_000_000);
        }
        let mut budget = Budget {
            limit: 5,
            last: None,
            blocked_until: i128::MAX - 1,
        };
        limiter.defer(&mut budget, Some("0"), 0);
        assert_eq!(budget.blocked_until, i128::MAX - 1);
    }
    #[test]
    fn fifo_cancelled_waiter_is_removed_and_retry_deadline_survives_cancellation() {
        let time = Arc::new(Time::default());
        let limiter = RateLimiter::new(Arc::<Time>::clone(&time), Arc::<Time>::clone(&time));
        let mut third = Box::pin(limiter.lock(Group::Stock));
        let original_limit = {
            let mut second = Box::pin(limiter.lock(Group::Stock));
            let first_limit = {
                let mut first = runtime().block_on(limiter.lock(Group::Stock));
                assert!(poll(second.as_mut()).is_pending());
                assert!(poll(third.as_mut()).is_pending());
                limiter.defer(&mut first, Some("9"), 1);
                first.limit
            };
            assert_eq!(first_limit, 5);
            assert!(poll(third.as_mut()).is_pending());
            first_limit
        };
        assert_eq!(original_limit, 5);
        let Poll::Ready(mut guard) = poll(third.as_mut()) else {
            panic!("third must advance")
        };
        assert!(admission_pending(&limiter, &mut guard));
        time.nanos.store(11_000_000_000, Ordering::SeqCst);
        assert!(!admission_pending(&limiter, &mut guard));
        assert_eq!(guard.last, Some(limiter.now()));
    }
    #[test]
    fn huge_retry_after_is_waited_in_bounded_chunks_without_overflow() {
        let time = Arc::new(Time::default());
        let limiter = RateLimiter::new(Arc::<Time>::clone(&time), Arc::<Time>::clone(&time));
        let mut budget = Budget {
            limit: 5,
            last: None,
            blocked_until: 0,
        };
        limiter.defer(&mut budget, Some("18446744073709551615"), 2);
        assert!(budget.blocked_until > limiter.now());
        assert!(admission_pending(&limiter, &mut budget));
        assert_eq!(time.duration.load(Ordering::SeqCst), 86_400);
    }
    #[test]
    fn zero_header_keeps_the_last_usable_capacity() {
        let mut budget = Budget {
            limit: 3,
            last: None,
            blocked_until: 0,
        };
        budget.observe(Some("0"));
        assert_eq!(budget.limit, 3);
    }

    fn spec_path(route: &ReadRoute) -> &'static str {
        match route {
            ReadRoute::Accounts => "/api/v1/accounts",
            ReadRoute::Holdings => "/api/v1/holdings",
            ReadRoute::Transactions => "/api/v1/transactions",
            ReadRoute::Quotes => "/api/v1/prices",
            ReadRoute::Candles => "/api/v1/candles",
            ReadRoute::BuyingPower => "/api/v1/buying-power",
            ReadRoute::SellableQuantity => "/api/v1/sellable-quantity",
            ReadRoute::Orders => "/api/v1/orders",
            ReadRoute::FxRate => "/api/v1/exchange-rate",
            ReadRoute::Orderbook => "/api/v1/orderbook",
            ReadRoute::PriceLimits => "/api/v1/price-limits",
            ReadRoute::CommissionRate => "/api/v1/commissions",
            ReadRoute::Instruments => "/api/v1/stocks/all",
            ReadRoute::Stocks => "/api/v1/stocks",
            ReadRoute::Rankings => "/api/v1/rankings",
            ReadRoute::InvestorRankings => "/api/v1/rankings/investor-trading",
            ReadRoute::IndicatorQuotes => "/api/v1/market-indicators/prices",
            ReadRoute::Sectors => "/api/v1/sectors",
            ReadRoute::SectorRankings => "/api/v1/sectors/rankings",
            ReadRoute::Order(_) => "/api/v1/orders/{orderId}",
            ReadRoute::StockWarnings(_) => "/api/v1/stocks/{symbol}/warnings",
            ReadRoute::StockInvestorFlow(_) => "/api/v1/stocks/{symbol}/investor-trading",
            ReadRoute::ProgramTrades(_) => "/api/v1/stocks/{symbol}/program-trades",
            ReadRoute::ShortSelling(_) => "/api/v1/stocks/{symbol}/short-selling",
            ReadRoute::CreditTrades(_) => "/api/v1/stocks/{symbol}/credit-trades",
            ReadRoute::SecuritiesLending(_) => "/api/v1/stocks/{symbol}/securities-lending",
            ReadRoute::IndicatorCandles(_) => "/api/v1/market-indicators/{symbol}/candles",
            ReadRoute::IndicatorInvestorFlow(_) => {
                "/api/v1/market-indicators/{symbol}/investor-trading"
            }
            ReadRoute::Sector(_) => "/api/v1/sectors/{sectorId}",
            ReadRoute::SectorStocks(_) => "/api/v1/sectors/{sectorId}/stocks",
            ReadRoute::SectorEtfs(_) => "/api/v1/sectors/{sectorId}/etfs",
            ReadRoute::CalendarKr => "/api/v1/market-calendar/KR",
            ReadRoute::CalendarUs => "/api/v1/market-calendar/US",
        }
    }
    fn label(group: Group) -> &'static str {
        match group {
            Group::Account => "ACCOUNT",
            Group::Asset => "ASSET",
            Group::Transaction => "TRANSACTION",
            Group::Stock => "STOCK",
            Group::StockAll => "STOCK_ALL",
            Group::StockTrend => "STOCK_TRADING_TREND",
            Group::MarketInfo => "MARKET_INFO",
            Group::MarketData => "MARKET_DATA",
            Group::Chart => "MARKET_DATA_CHART",
            Group::Ranking => "RANKING",
            Group::Indicator => "MARKET_INDICATOR",
            Group::IndicatorChart => "MARKET_INDICATOR_CHART",
            Group::Sector => "SECTOR",
            Group::SectorRanking => "SECTOR_RANKING",
            Group::OrderHistory => "ORDER_HISTORY",
            Group::OrderInfo => "ORDER_INFO",
        }
    }
    fn all_read_routes() -> [ReadRoute; 33] {
        let id = crate::RouteId::new("sample").expect("route ID");
        [
            ReadRoute::Accounts,
            ReadRoute::Holdings,
            ReadRoute::Transactions,
            ReadRoute::Quotes,
            ReadRoute::Candles,
            ReadRoute::BuyingPower,
            ReadRoute::SellableQuantity,
            ReadRoute::Orders,
            ReadRoute::FxRate,
            ReadRoute::Orderbook,
            ReadRoute::PriceLimits,
            ReadRoute::CommissionRate,
            ReadRoute::Instruments,
            ReadRoute::Stocks,
            ReadRoute::Rankings,
            ReadRoute::InvestorRankings,
            ReadRoute::IndicatorQuotes,
            ReadRoute::Sectors,
            ReadRoute::SectorRankings,
            ReadRoute::Order(id.clone()),
            ReadRoute::StockWarnings(id.clone()),
            ReadRoute::StockInvestorFlow(id.clone()),
            ReadRoute::ProgramTrades(id.clone()),
            ReadRoute::ShortSelling(id.clone()),
            ReadRoute::CreditTrades(id.clone()),
            ReadRoute::SecuritiesLending(id.clone()),
            ReadRoute::IndicatorCandles(id.clone()),
            ReadRoute::IndicatorInvestorFlow(id.clone()),
            ReadRoute::Sector(id.clone()),
            ReadRoute::SectorStocks(id.clone()),
            ReadRoute::SectorEtfs(id),
            ReadRoute::CalendarKr,
            ReadRoute::CalendarUs,
        ]
    }
    #[test]
    fn every_read_route_uses_its_openapi_group_and_path() {
        use crate::ReadRequest;
        let spec: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string("../../../docs/toss-openapi-1.2.28.json").expect("spec"),
        )
        .expect("JSON");
        let origin = reqwest::Url::parse("http://127.0.0.1:49123").expect("origin");
        let routes = all_read_routes();
        assert_eq!(routes.len(), 33);
        for route in routes {
            let path = spec_path(&route);
            let operation = spec
                .get("paths")
                .and_then(|paths| paths.get(path))
                .and_then(|item| item.get("get"))
                .expect("documented GET");
            let description = operation
                .get("description")
                .and_then(serde_json::Value::as_str)
                .expect("description");
            let expected = description
                .split("**Rate Limits Group**: `")
                .nth(1)
                .and_then(|tail| tail.split('`').next())
                .expect("group");
            assert_eq!(label(Group::route(&route)), expected, "{path}");
            let expanded = path
                .replace("{symbol}", "sample")
                .replace("{orderId}", "sample")
                .replace("{sectorId}", "sample");
            assert_eq!(ReadRequest::new(route).url(&origin).path(), expanded);
        }
    }
    #[test]
    fn noncancelled_waiters_keep_poll_registration_order() {
        let time = Arc::new(Time::default());
        let limiter = RateLimiter::new(Arc::<Time>::clone(&time), time);
        let mut second = Box::pin(limiter.lock(Group::Stock));
        let mut third = Box::pin(limiter.lock(Group::Stock));
        let initial_limit = {
            let first = runtime().block_on(limiter.lock(Group::Stock));
            assert!(poll(second.as_mut()).is_pending());
            assert!(poll(third.as_mut()).is_pending());
            first.limit
        };
        assert_eq!(initial_limit, 5);
        assert!(poll(third.as_mut()).is_pending());
        let second_limit = {
            let Poll::Ready(second_guard) = poll(second.as_mut()) else {
                panic!("second must be first")
            };
            assert!(poll(third.as_mut()).is_pending());
            second_guard.limit
        };
        assert_eq!(second_limit, 5);
        assert!(poll(third.as_mut()).is_ready());
    }
}
