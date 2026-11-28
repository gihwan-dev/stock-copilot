use crate::{
    BriefingContext, BriefingContextSource, MarketOverviewToolView, MarketToolReads, OverviewItem,
    OverviewToolLine, OverviewUnit, SessionStatus, SystemPortError, ValueProvenance,
};
use domain::{
    BriefingKind, DisplayPrecision, KnownMarket, Timestamp, UntrustedText, display_number,
    display_percent,
};
use std::fmt::{self, Debug, Formatter};

pub struct MarketBriefingContext<'a, Inner, Market> {
    pub inner: &'a Inner,
    pub market: &'a Market,
}

impl<Inner, Market> Debug for MarketBriefingContext<'_, Inner, Market> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("MarketBriefingContext")
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Inclusion {
    Included,
    Skipped,
}

const fn us_item(item: OverviewItem) -> Inclusion {
    match item {
        OverviewItem::Sp500
        | OverviewItem::Nasdaq100
        | OverviewItem::UsTreasury2Y
        | OverviewItem::UsTreasury10Y
        | OverviewItem::UsdKrw
        | OverviewItem::DollarIndex
        | OverviewItem::CrudeOil
        | OverviewItem::Gold => Inclusion::Included,
        OverviewItem::Kospi
        | OverviewItem::Kosdaq
        | OverviewItem::KrBond3Y
        | OverviewItem::KrBond10Y => Inclusion::Skipped,
    }
}

const fn wanted(kind: BriefingKind, item: OverviewItem) -> Inclusion {
    match kind {
        BriefingKind::Morning => Inclusion::Included,
        BriefingKind::PreUsMarket => us_item(item),
        BriefingKind::ClosingReview => Inclusion::Skipped,
    }
}

const fn wanted_session(kind: BriefingKind, market: KnownMarket) -> Inclusion {
    match (kind, market) {
        (BriefingKind::Morning, _) | (BriefingKind::PreUsMarket, KnownMarket::Us) => {
            Inclusion::Included
        }
        (BriefingKind::PreUsMarket, KnownMarket::Kr) | (BriefingKind::ClosingReview, _) => {
            Inclusion::Skipped
        }
    }
}

#[must_use]
pub fn market_overview_line_fact(line: &OverviewToolLine) -> Option<UntrustedText> {
    let precision = DisplayPrecision::try_new(2).ok()?;
    let value = display_number(&line.value, precision);
    let shown = match line.unit {
        OverviewUnit::IndexPoints => value.as_str().to_owned(),
        OverviewUnit::YieldPercent => format!("{}%", value.as_str()),
        OverviewUnit::KrwPerUsd => format!("{}원", value.as_str()),
        OverviewUnit::UsdPrice => format!("${}", value.as_str()),
    };
    let change = line.change_rate.as_ref().map_or_else(String::new, |rate| {
        let digits = display_percent(rate, precision);
        let sign = if rate.as_decimal().is_sign_negative() || rate.as_decimal().is_zero() {
            ""
        } else {
            "+"
        };
        format!(" ({sign}{}%)", digits.as_str())
    });
    let marker = match (line.provenance, line.unit) {
        (ValueProvenance::Approximate, OverviewUnit::UsdPrice) => " [ETF 근사값]",
        (ValueProvenance::Approximate, _) => " [근사값]",
        (ValueProvenance::Official, _) => "",
    };
    let label = match line.item {
        OverviewItem::Kospi => "코스피",
        OverviewItem::Kosdaq => "코스닥",
        OverviewItem::KrBond3Y => "국고채 3년",
        OverviewItem::KrBond10Y => "국고채 10년",
        OverviewItem::UsdKrw => "원/달러",
        OverviewItem::Sp500 => "미국 S&P500",
        OverviewItem::Nasdaq100 => "미국 나스닥100",
        OverviewItem::UsTreasury2Y => "미국채 2년",
        OverviewItem::UsTreasury10Y => "미국채 10년",
        OverviewItem::CrudeOil => "원유",
        OverviewItem::Gold => "금",
        OverviewItem::DollarIndex => "달러 인덱스",
    };
    UntrustedText::try_new(&format!("시장: {label} {shown}{change}{marker}")).ok()
}

fn session_fact(market: KnownMarket, status: SessionStatus) -> Option<UntrustedText> {
    let place = match market {
        KnownMarket::Kr => "한국장",
        KnownMarket::Us => "미국장",
    };
    let state = match status {
        SessionStatus::PreMarket => "장 시작 전",
        SessionStatus::Open => "장 중",
        SessionStatus::AfterHours => "시간외",
        SessionStatus::Closed => "장 마감",
        SessionStatus::Holiday => "휴장",
    };
    UntrustedText::try_new(&format!("오늘 일정: {place} {state}")).ok()
}

#[must_use]
pub fn market_briefing_facts(
    kind: BriefingKind,
    view: &MarketOverviewToolView,
) -> Vec<UntrustedText> {
    let lines = view
        .lines
        .iter()
        .filter(|line| wanted(kind, line.item) == Inclusion::Included)
        .filter_map(market_overview_line_fact);
    let sessions = view
        .sessions
        .iter()
        .filter(|session| wanted_session(kind, session.market) == Inclusion::Included)
        .filter_map(|session| session_fact(session.market, session.status));
    lines.chain(sessions).collect()
}

impl<Inner: BriefingContextSource, Market: MarketToolReads> BriefingContextSource
    for MarketBriefingContext<'_, Inner, Market>
{
    async fn context(
        &self,
        kind: BriefingKind,
        now: Timestamp,
    ) -> Result<BriefingContext, SystemPortError> {
        let mut context = self.inner.context(kind, now).await?;
        if kind == BriefingKind::ClosingReview {
            return Ok(context);
        }
        if let Ok(Some(view)) = self.market.market_overview().await {
            context
                .code_facts
                .extend(market_briefing_facts(kind, &view));
        }
        Ok(context)
    }
}
