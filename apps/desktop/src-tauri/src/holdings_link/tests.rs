use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use application::{
    HttpStatus, PortError, PortErrorKind, ReadEndpoint, RetryDelay, UnavailableCause,
};
use domain::{
    CommonShare, Currency, DisplayInstrumentName, InstrumentId, InstrumentMaster,
    InstrumentMasterEntry, KnownMarket, KoreanMasterDetail, LiquidationTrading, ListingStatus,
    MasterCode, MasterInstrument, MasterInstrumentFacts, MasterInstrumentRow, MasterSecurityType,
    Nanosecond, NxtTrading, OffsetSeconds, SpanSeconds, Symbol, TimeSpan, Timestamp,
    TradingSuspension, UnixSeconds,
};
use testkit_ports::SteppedClock;

use super::{
    HoldingsListener, HoldingsSync, InstrumentDirectory, InstrumentMasterFeed, MasterRefresh,
    RefreshOutcome,
};
use crate::ipc::state::BoxFuture;
use crate::toss_link::NoListener;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(32_400),
    )
    .expect("시각")
}

fn instrument(market: KnownMarket, symbol: &str, name: &str) -> MasterInstrument {
    let (listing, currency, korean_detail) = match market {
        KnownMarket::Kr => (
            "KOSPI",
            Currency::Krw,
            Some(KoreanMasterDetail {
                liquidation: LiquidationTrading::Regular,
                krx: TradingSuspension::Clear,
                nxt: NxtTrading::Unsupported,
            }),
        ),
        KnownMarket::Us => ("NASDAQ", Currency::Usd, None),
    };
    MasterInstrument::try_new(MasterInstrumentRow {
        listing_market: MasterCode::try_new(listing).expect("시장"),
        symbol: Symbol::try_new(symbol).expect("심볼"),
        instrument_id: InstrumentId::try_new(&format!("inst-{symbol}")).expect("ID"),
        name: DisplayInstrumentName::try_new(name).expect("이름"),
        security_type: MasterSecurityType::from_code(MasterCode::try_new("STOCK").expect("유형")),
        currency,
        listing_status: ListingStatus::Active,
        common_share: CommonShare::Common,
        korean_detail,
        observed_at: at(1_790_000_000),
    })
    .expect("마스터")
}

fn unavailable() -> PortError {
    PortError::new(
        ReadEndpoint::Instruments,
        HttpStatus::NotReceived,
        PortErrorKind::Unavailable {
            cause: UnavailableCause::Network,
            retry: RetryDelay::Unknown,
        },
    )
}

#[derive(Default)]
struct FakeFeed {
    fetches: AtomicUsize,
    us_failures: AtomicUsize,
    installed: std::sync::RwLock<Vec<MasterInstrumentFacts>>,
}

impl InstrumentMasterFeed for FakeFeed {
    fn fetch(&self, market: KnownMarket) -> BoxFuture<'_, Result<InstrumentMaster, PortError>> {
        self.fetches.fetch_add(1, Ordering::SeqCst);
        let result = match market {
            KnownMarket::Kr => InstrumentMaster::try_new(vec![InstrumentMasterEntry::Available(
                instrument(KnownMarket::Kr, "005930", "삼성전자"),
            )])
            .map_err(|_error| unavailable()),
            KnownMarket::Us => {
                if self.us_failures.load(Ordering::SeqCst) > 0 {
                    self.us_failures.fetch_sub(1, Ordering::SeqCst);
                    Err(unavailable())
                } else {
                    InstrumentMaster::try_new(vec![InstrumentMasterEntry::Available(instrument(
                        KnownMarket::Us,
                        "AAPL",
                        "애플",
                    ))])
                    .map_err(|_error| unavailable())
                }
            }
        };
        Box::pin(std::future::ready(result))
    }

    fn install(&self, facts: MasterInstrumentFacts) {
        self.installed.write().expect("installed").push(facts);
    }
}

#[derive(Default)]
struct CountingSync(AtomicUsize);

impl HoldingsSync for CountingSync {
    fn sync(&self) -> BoxFuture<'_, ()> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::ready(()))
    }
}

fn day() -> TimeSpan {
    TimeSpan::try_new(SpanSeconds::new(86_400), Nanosecond::new(0)).expect("하루")
}

#[test]
fn master_refresh_installs_both_markets_once_a_day_and_fills_names() {
    let feed = Arc::new(FakeFeed::default());
    let clock = Arc::new(SteppedClock::new(at(1_790_000_000)));
    let directory = InstrumentDirectory::default();
    let refresh = MasterRefresh::new(
        Arc::<FakeFeed>::clone(&feed),
        directory.clone(),
        Arc::<SteppedClock>::clone(&clock),
    );
    let sync = Arc::new(CountingSync::default());
    let listener = HoldingsListener::new(
        Arc::new(NoListener),
        Arc::new(refresh),
        Arc::<CountingSync>::clone(&sync),
    );
    tauri::async_runtime::block_on(async {
        assert_eq!(listener.refresh_holdings().await, RefreshOutcome::Refreshed);
        assert_eq!(feed.fetches.load(Ordering::SeqCst), 2);
        assert_eq!(sync.0.load(Ordering::SeqCst), 1);
        let installed = feed.installed.read().expect("installed").clone();
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].instruments().len(), 2);
        assert_eq!(
            directory
                .name(&Symbol::try_new("005930").expect("심볼"))
                .map(|name| name.as_str().to_owned()),
            Some(String::from("삼성전자"))
        );
        assert_eq!(
            directory
                .entry(&Symbol::try_new("AAPL").expect("심볼"))
                .map(|entry| entry.market),
            Some(KnownMarket::Us)
        );
        assert_eq!(listener.refresh_holdings().await, RefreshOutcome::Fresh);
        assert_eq!(feed.fetches.load(Ordering::SeqCst), 2);
        assert_eq!(sync.0.load(Ordering::SeqCst), 1);
        let _next = clock.advance_by(day()).expect("하루 뒤");
        assert_eq!(listener.refresh_holdings().await, RefreshOutcome::Refreshed);
        assert_eq!(feed.fetches.load(Ordering::SeqCst), 4);
        assert_eq!(sync.0.load(Ordering::SeqCst), 2);
    });
}

#[test]
fn failed_market_keeps_the_loaded_one_and_is_retried_without_refetching_the_other() {
    let feed = Arc::new(FakeFeed::default());
    feed.us_failures.store(1, Ordering::SeqCst);
    let clock = Arc::new(SteppedClock::new(at(1_790_000_000)));
    let directory = InstrumentDirectory::default();
    let refresh = MasterRefresh::new(
        Arc::<FakeFeed>::clone(&feed),
        directory.clone(),
        Arc::<SteppedClock>::clone(&clock),
    );
    tauri::async_runtime::block_on(async {
        assert_eq!(refresh.refresh_due().await, RefreshOutcome::Refreshed);
        assert_eq!(
            feed.installed.read().expect("installed")[0]
                .instruments()
                .len(),
            1
        );
        assert!(
            directory
                .name(&Symbol::try_new("AAPL").expect("심볼"))
                .is_none()
        );
        assert_eq!(refresh.refresh_due().await, RefreshOutcome::Refreshed);
        assert_eq!(feed.fetches.load(Ordering::SeqCst), 3);
        let installed = feed.installed.read().expect("installed").clone();
        assert_eq!(installed.len(), 2);
        assert_eq!(installed[1].instruments().len(), 2);
        feed.us_failures.store(1, Ordering::SeqCst);
        let _next = clock.advance_by(day()).expect("하루 뒤");
        assert_eq!(refresh.refresh_due().await, RefreshOutcome::Refreshed);
        assert_eq!(
            feed.installed.read().expect("installed")[2]
                .instruments()
                .len(),
            2
        );
    });
}

#[test]
fn nothing_installed_when_every_market_fails() {
    struct Failing;
    impl InstrumentMasterFeed for Failing {
        fn fetch(
            &self,
            _market: KnownMarket,
        ) -> BoxFuture<'_, Result<InstrumentMaster, PortError>> {
            Box::pin(std::future::ready(Err(unavailable())))
        }
        fn install(&self, _facts: MasterInstrumentFacts) {
            panic!("설치하면 안 됨");
        }
    }
    let refresh = MasterRefresh::new(
        Arc::new(Failing),
        InstrumentDirectory::default(),
        Arc::new(SteppedClock::new(at(1_790_000_000))),
    );
    assert_eq!(
        tauri::async_runtime::block_on(refresh.refresh_due()),
        RefreshOutcome::Failed
    );
}
