use super::fact;
use application::{
    BriefingSource, CandlePriceBasis, InvestorFlowToolView, PricedCandlesView, TrackedInstrument,
};
use domain::{Currency, KnownMarket, Money, Timestamp};

pub(super) fn recent(at: Timestamp, now: Timestamp) -> bool {
    at <= now
        && now
            .utc_seconds()
            .as_i64()
            .checked_sub(*at.utc_seconds().as_i64())
            .is_some_and(|age| age <= 604_800)
}

pub(super) const fn region(market: KnownMarket) -> &'static str {
    match market {
        KnownMarket::Kr => "한국",
        KnownMarket::Us => "미국",
    }
}

pub(super) fn money(value: Money) -> String {
    let unit = match value.currency() {
        Currency::Krw => "원",
        Currency::Usd => "달러",
    };
    format!("{} {unit}", value.amount().as_decimal())
}

pub(super) fn candles(
    view: &PricedCandlesView,
    target: &TrackedInstrument,
    at: Timestamp,
) -> Option<BriefingSource> {
    let series = &view.view;
    if series.symbol != target.symbol
        || series.market != target.market
        || !recent(series.collected_at, at)
    {
        return None;
    }
    let mut days = series
        .lines
        .iter()
        .filter(|day| recent(day.opened_at, series.collected_at))
        .collect::<Vec<_>>();
    days.sort_by_key(|day| day.opened_at);
    let latest = days.last()?;
    let (Some(first), basis) = (
        days.first(),
        match view.basis {
            CandlePriceBasis::Adjusted => "수정",
            CandlePriceBasis::Unadjusted => "비수정",
        },
    ) else {
        return None;
    };
    let start = crate::ipc::briefing::kst_iso(first.opened_at)?;
    let end = crate::ipc::briefing::kst_iso(latest.opened_at)?;
    let text = if first.opened_at == latest.opened_at {
        format!(
            "토스증권 {} {basis} 확정 일봉 · {end} 시작 봉 종가 {}",
            target.symbol.as_str(),
            money(latest.close)
        )
    } else {
        format!(
            "토스증권 {} {basis} 확정 일봉 · {start} 시작 봉 종가 {} → {end} 시작 봉 종가 {}",
            target.symbol.as_str(),
            money(first.close),
            money(latest.close)
        )
    };
    fact(text, series.collected_at)
}

pub(super) fn flows(
    view: &InvestorFlowToolView,
    target: &TrackedInstrument,
    at: Timestamp,
) -> Option<BriefingSource> {
    if view.symbol != target.symbol
        || view.market != target.market
        || !recent(view.collected_at, at)
    {
        return None;
    }
    let day = view
        .lines
        .iter()
        .filter(|day| recent(day.trading_date, view.collected_at))
        .filter(|day| day.foreign_net.is_some() || day.institution_net.is_some())
        .max_by_key(|day| day.trading_date)?;
    let metrics = [("외국인", day.foreign_net), ("기관", day.institution_net)]
        .into_iter()
        .filter_map(|(label, value)| Some(format!("{label} 순매수 {}", money(value?))))
        .collect::<Vec<_>>()
        .join(", ");
    fact(
        format!(
            "토스증권 {} 투자자 수급 · {} · {metrics}",
            target.symbol.as_str(),
            crate::ipc::briefing::kst_iso(day.trading_date)?
        ),
        view.collected_at,
    )
}
