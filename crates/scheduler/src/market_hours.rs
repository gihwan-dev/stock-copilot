use crate::types::ScheduleRule;
use domain::{KnownMarket, MarketCalendar, MarketState, MarketTradingStatus, TimeSpan, Timestamp};

pub trait MarketCalendarSource: Send + Sync {
    fn calendar(&self, market: KnownMarket) -> Option<MarketCalendar>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoMarketCalendar;

impl MarketCalendarSource for NoMarketCalendar {
    fn calendar(&self, _market: KnownMarket) -> Option<MarketCalendar> {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DueBound {
    AtOrAfter,
    StrictlyAfter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Boundary {
    Open,
    Close,
}

pub(crate) fn market_due(
    source: &dyn MarketCalendarSource,
    rule: ScheduleRule,
    after: Timestamp,
    bound: DueBound,
) -> Option<Timestamp> {
    let (market, boundary, shift) = match rule {
        ScheduleRule::AfterMarketClose { market, delay, .. } => (market, Boundary::Close, delay),
        ScheduleRule::BeforeMarketOpen { market, lead, .. } => (market, Boundary::Open, lead),
        ScheduleRule::DailyKst(_)
        | ScheduleRule::WeeklyKst { .. }
        | ScheduleRule::Interval(_)
        | ScheduleRule::EventTriggered => return None,
    };
    let calendar = source.calendar(market)?;
    regular_boundaries(&calendar, boundary)
        .into_iter()
        .filter_map(|at| shifted(at, boundary, shift))
        .filter(|at| match bound {
            DueBound::AtOrAfter => *at >= after,
            DueBound::StrictlyAfter => *at > after,
        })
        .min()
}

#[must_use]
pub fn trading_status_now(
    source: &dyn MarketCalendarSource,
    now: Timestamp,
) -> MarketTradingStatus {
    let mut status = MarketTradingStatus::Closed;
    for market in [KnownMarket::Kr, KnownMarket::Us] {
        match source
            .calendar(market)
            .map(|calendar| calendar.state_at(now))
        {
            None | Some(MarketState::Unknown) => status = MarketTradingStatus::Unknown,
            Some(state) => {
                if state.trading_status() == MarketTradingStatus::Open {
                    return MarketTradingStatus::Open;
                }
            }
        }
    }
    status
}

fn regular_boundaries(calendar: &MarketCalendar, boundary: Boundary) -> Vec<Timestamp> {
    [
        calendar.previous_business_day(),
        calendar.today(),
        calendar.next_business_day(),
    ]
    .iter()
    .flat_map(|session| session.sessions().iter())
    .filter(|segment| segment.state() == MarketState::RegularMarket)
    .map(|segment| {
        let interval = segment.session().interval();
        match boundary {
            Boundary::Open => interval.start(),
            Boundary::Close => interval.end(),
        }
    })
    .collect()
}

fn shifted(at: Timestamp, boundary: Boundary, shift: TimeSpan) -> Option<Timestamp> {
    match boundary {
        Boundary::Open => at.checked_sub(shift).ok(),
        Boundary::Close => at.checked_add(shift).ok(),
    }
}
