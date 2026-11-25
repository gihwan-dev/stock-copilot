use super::{CalendarDays, EstimateRunBook, ProductMaterials, task_calendars, trade_tax};
use crate::broker_link::AccountSlot;
use crate::holdings_link::InstrumentDirectory;
use crate::ipc::state::BoxFuture;
use application::{
    EstimateRunContextSource, EstimateRunRegistry, EstimateTaskMaterialSource, PortError,
    QuoteObservation, TrackingTarget,
};
use domain::{
    AccountId, CalendarDate, CommissionRateFact, CommissionValidity, CostRate, Currency,
    KnownMarket, Market, MarketCalendar, NumericInput, Price, Quote, Ratio,
};
use std::future::Future;
use std::sync::{Arc, RwLock};
use testkit_ports::estimate_fixtures::{
    sample_cost_eligible_task, sample_daily_candles, sample_day, sample_estimate_run_context,
    sample_kr_calendars, sample_local, sample_master,
};
use testkit_ports::memory::{MemoryReply, MemorySettingsRepository};
use testkit_ports::scripted::ScriptedBrokerRead;

fn block<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("런타임")
        .block_on(future)
}

struct Days {
    calendars: Vec<MarketCalendar>,
    calls: RwLock<Vec<CalendarDate>>,
}

impl CalendarDays for Days {
    fn calendar_on(
        &self,
        _market: KnownMarket,
        date: CalendarDate,
    ) -> BoxFuture<'_, Result<MarketCalendar, PortError>> {
        self.calls.write().expect("기록").push(date);
        let found = self
            .calendars
            .iter()
            .find(|calendar| calendar.today().date() == date)
            .cloned()
            .ok_or(PortError::new(
                application::ReadEndpoint::Calendar,
                application::HttpStatus::NotReceived,
                application::PortErrorKind::Unavailable {
                    cause: application::UnavailableCause::Network,
                    retry: application::RetryDelay::Unknown,
                },
            ));
        Box::pin(std::future::ready(found))
    }
}

fn days(last: i64) -> Arc<Days> {
    Arc::new(Days {
        calendars: sample_kr_calendars(58, last).expect("달력"),
        calls: RwLock::new(Vec::new()),
    })
}

#[test]
fn run_book_serves_registered_context_until_release() {
    let book = EstimateRunBook::new();
    let context = sample_estimate_run_context("run-a").expect("맥락");
    book.register(context.clone());
    assert_eq!(
        block(book.estimate_context(&context.run)),
        Ok(context.clone())
    );
    book.release(&context.run);
    assert_eq!(
        block(book.estimate_context(&context.run)),
        Err(super::unavailable())
    );
}

#[test]
fn tax_schedule_is_zero_on_buy_and_market_specific_on_sell() {
    let rate = |text: &str| {
        CostRate::try_new(Ratio::parse(&NumericInput::try_new(text).expect("입력")).expect("비율"))
            .expect("요율")
    };
    let kr = trade_tax(KnownMarket::Kr).expect("국내");
    assert_eq!(kr.market(), &Market::Kr);
    assert_eq!(kr.buy_rate(), rate("0"));
    assert_eq!(kr.sell_rate(), rate("0.0020"));
    let us = trade_tax(KnownMarket::Us).expect("미국");
    assert_eq!(us.sell_rate(), rate("0.0002"));
}

#[test]
fn task_calendars_start_at_the_reference_day_and_cover_the_maturity() {
    let eligible = sample_cost_eligible_task().expect("과제");
    let task = eligible.task();
    let source = days(80);
    let chain = block(task_calendars(source.as_ref(), task)).expect("달력");
    assert_eq!(chain.len(), 8);
    assert_eq!(
        chain.first().expect("첫날").today().date(),
        task.reference().observed_at().offset_calendar_date()
    );
    assert_eq!(
        domain::estimate_maturity(task, &chain).map(|_maturity| ()),
        Ok(())
    );
    let short = days(62);
    assert_eq!(
        block(task_calendars(short.as_ref(), task)).map(|found| found.len()),
        Err(super::unavailable())
    );
}

fn price(text: &str) -> Price {
    Price::parse(&NumericInput::try_new(text).expect("입력"), Currency::Krw).expect("가격")
}

fn materials(
    directory: InstrumentDirectory,
    accounts: AccountSlot,
) -> ProductMaterials<ScriptedBrokerRead, MemorySettingsRepository> {
    ProductMaterials {
        broker: Arc::new(ScriptedBrokerRead::default()),
        settings: Arc::new(MemorySettingsRepository::default()),
        directory,
        accounts,
        days: days(80),
    }
}

#[test]
fn materials_need_a_known_master_and_a_saved_account() {
    let master = sample_master().expect("마스터");
    let target = TrackingTarget::for_holding(master.row().instrument_id.clone());
    let now = sample_day(62).expect("시각");
    let empty = materials(
        InstrumentDirectory::default(),
        AccountSlot::with(vec![AccountId::new(7)]),
    );
    assert_eq!(block(empty.materials(&target, now)), Ok(None));
    let directory = InstrumentDirectory::default();
    directory.replace(std::slice::from_ref(&master));
    let no_account = materials(directory, AccountSlot::default());
    assert_eq!(block(no_account.materials(&target, now)), Ok(None));
    assert_eq!(no_account.broker.calls().len(), 0);
}

#[test]
fn materials_use_the_live_quote_settled_daily_series_commission_and_tax() {
    let master = sample_master().expect("마스터");
    let target = TrackingTarget::for_holding(master.row().instrument_id.clone());
    let now = sample_day(62).expect("시각");
    let directory = InstrumentDirectory::default();
    directory.replace(std::slice::from_ref(&master));
    let source = materials(directory, AccountSlot::with(vec![AccountId::new(7)]));
    let quote_instrument = master.quote_instrument().expect("시세 종목");
    source.settings.script_profile().push(MemoryReply::Memory);
    source
        .broker
        .script_quotes()
        .push(Ok(vec![QuoteObservation {
            instrument: quote_instrument,
            result: Ok(Quote::try_new(
                master.row().symbol.clone(),
                Currency::Krw,
                Some(price("101")),
                Some(sample_local(now).expect("현지")),
                now,
            )
            .expect("시세")),
        }]));
    source
        .broker
        .script_candles()
        .push(Ok(sample_daily_candles(63).expect("일봉")));
    source
        .broker
        .script_commission_rate()
        .push(Ok(CommissionRateFact::from_commissions_result(
            Market::Kr,
            CostRate::try_new(
                Ratio::parse(&NumericInput::try_new("0.00015").expect("입력")).expect("비율"),
            )
            .expect("요율"),
            CommissionValidity::try_new(None, None).expect("유효"),
        )));
    let found = block(source.materials(&target, now))
        .expect("재료")
        .expect("있음");
    let reference = master
        .price_observation(price("101"), sample_local(now).expect("현지"))
        .expect("기준가");
    assert_eq!(found.context, domain::EstimateContext::new(reference, now));
    assert_eq!(found.volatility.instrument(), &master.row().instrument_id);
    assert!(found.commission.is_some());
    assert_eq!(found.tax, trade_tax(KnownMarket::Kr));
    let calls = source.broker.calls();
    assert!(calls.iter().any(|call| matches_candles(call, now)));
}

fn matches_candles(call: &testkit_ports::scripted::BrokerReadCall, now: domain::Timestamp) -> bool {
    match call {
        testkit_ports::scripted::BrokerReadCall::Candles(_, window, period, adjustment) => {
            window.end() == now
                && *period == domain::CandlePeriod::Daily
                && *adjustment
                    == domain::PriceAdjustment::Adjusted(domain::AdjustmentBasis::new(now))
        }
        testkit_ports::scripted::BrokerReadCall::Accounts(..)
        | testkit_ports::scripted::BrokerReadCall::Holdings(_)
        | testkit_ports::scripted::BrokerReadCall::Transactions(..)
        | testkit_ports::scripted::BrokerReadCall::Quotes(_)
        | testkit_ports::scripted::BrokerReadCall::BuyingPower(..)
        | testkit_ports::scripted::BrokerReadCall::SellableQuantity(..)
        | testkit_ports::scripted::BrokerReadCall::OpenOrders(_)
        | testkit_ports::scripted::BrokerReadCall::ClosedOrders(..)
        | testkit_ports::scripted::BrokerReadCall::Order(..)
        | testkit_ports::scripted::BrokerReadCall::StockWarnings(_)
        | testkit_ports::scripted::BrokerReadCall::Instruments(_)
        | testkit_ports::scripted::BrokerReadCall::Orderbook(_)
        | testkit_ports::scripted::BrokerReadCall::PriceLimits(_)
        | testkit_ports::scripted::BrokerReadCall::CommissionRate(..) => false,
    }
}
