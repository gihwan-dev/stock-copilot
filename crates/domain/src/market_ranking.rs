use crate::{
    Currency, KnownMarket, MarketDataError, MarketObservation, MasterCode, Money, NumericInput,
    Quantity, Ratio, Symbol, Timestamp,
};
use std::collections::BTreeSet;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankingKind {
    MarketTradingAmount,
    MarketTradingVolume,
    TossTradingAmount,
    TossTradingVolume,
    TopGainers,
    TopLosers,
    Unknown,
}
impl RankingKind {
    #[must_use]
    pub fn from_code(code: &MasterCode) -> Self {
        match code.as_str() {
            "MARKET_TRADING_AMOUNT" => Self::MarketTradingAmount,
            "MARKET_TRADING_VOLUME" => Self::MarketTradingVolume,
            "TOSS_SECURITIES_TRADING_AMOUNT" => Self::TossTradingAmount,
            "TOSS_SECURITIES_TRADING_VOLUME" => Self::TossTradingVolume,
            "TOP_GAINERS" => Self::TopGainers,
            "TOP_LOSERS" => Self::TopLosers,
            _ => Self::Unknown,
        }
    }
    #[must_use]
    pub const fn unit(self) -> Option<RankingUnit> {
        match self {
            Self::MarketTradingAmount | Self::TossTradingAmount => Some(RankingUnit::Money),
            Self::MarketTradingVolume | Self::TossTradingVolume => Some(RankingUnit::Shares),
            Self::TopGainers | Self::TopLosers => Some(RankingUnit::Ratio),
            Self::Unknown => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankingDuration {
    Realtime,
    Day,
    Week,
    Month,
    ThreeMonths,
    SixMonths,
    Year,
    Unknown,
}
impl RankingDuration {
    #[must_use]
    pub fn from_code(code: &MasterCode) -> Self {
        match code.as_str() {
            "realtime" => Self::Realtime,
            "1d" => Self::Day,
            "1w" => Self::Week,
            "1mo" => Self::Month,
            "3mo" => Self::ThreeMonths,
            "6mo" => Self::SixMonths,
            "1y" => Self::Year,
            _ => Self::Unknown,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankingUnit {
    Shares,
    Money,
    Ratio,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankingCoverage {
    TopN,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RankingOrdinal(usize);
impl RankingOrdinal {
    #[must_use]
    pub const fn new(value: usize) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn as_usize(&self) -> &usize {
        &self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankingLimit(usize);
impl RankingLimit {
    pub fn try_new(value: usize) -> Result<Self, MarketDataError> {
        if !(1..=100).contains(&value) {
            return Err(MarketDataError::InvalidCount);
        }
        Ok(Self(value))
    }
    #[must_use]
    pub const fn as_usize(&self) -> &usize {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankingRow {
    pub rank: RankingOrdinal,
    pub symbol: Symbol,
    pub currency: Currency,
    pub volume: NumericInput,
    pub amount: NumericInput,
    pub change_rate: Option<NumericInput>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankingBasis {
    Shares(Quantity),
    Amount(Money),
    ChangeRate(Ratio),
}
impl RankingBasis {
    #[must_use]
    pub const fn unit(self) -> RankingUnit {
        match self {
            Self::Shares(_) => RankingUnit::Shares,
            Self::Amount(_) => RankingUnit::Money,
            Self::ChangeRate(_) => RankingUnit::Ratio,
        }
    }
    fn number(self) -> Ratio {
        match self {
            Self::Shares(value) => Ratio::new(value.as_decimal()),
            Self::Amount(value) => Ratio::new(value.amount().as_decimal()),
            Self::ChangeRate(value) => value,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankingItem {
    rank: RankingOrdinal,
    symbol: Symbol,
    basis: RankingBasis,
    change_rate: RankingChange,
}
impl RankingItem {
    #[must_use]
    pub const fn rank(&self) -> RankingOrdinal {
        self.rank
    }
    #[must_use]
    pub const fn symbol(&self) -> &Symbol {
        &self.symbol
    }
    #[must_use]
    pub const fn change_rate(&self) -> RankingChange {
        self.change_rate
    }
    #[must_use]
    pub const fn basis(&self) -> RankingBasis {
        self.basis
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockRanking {
    kind: RankingKind,
    market: KnownMarket,
    duration: RankingDuration,
    limit: RankingLimit,
    items: Vec<RankingItem>,
    observation: MarketObservation,
    coverage: RankingCoverage,
    continuity: RankingContinuity,
    caution_filter: RankingCautionFilter,
}
impl StockRanking {
    pub fn from_response(input: RankingResponseInput) -> Result<Self, MarketDataError> {
        let kind = RankingKind::from_code(&input.kind);
        let duration = RankingDuration::from_code(&input.duration);
        let RankingResponseInput {
            market,
            limit,
            rows,
            ranked_at,
            collected,
            caution_filter,
            ..
        } = input;
        let unit = kind.unit().ok_or(MarketDataError::UnsupportedRanking)?;
        if duration == RankingDuration::Unknown
            || (duration == RankingDuration::Realtime
                && (kind == RankingKind::TopGainers || kind == RankingKind::TopLosers))
        {
            return Err(MarketDataError::UnsupportedPeriod);
        }
        if rows.len() > limit.0 {
            return Err(MarketDataError::InvalidCount);
        }
        let mut items = rows
            .into_iter()
            .map(|row| ranking_item(row, market, unit))
            .collect::<Result<Vec<_>, _>>()?;
        items.sort_by_key(|row| row.rank);
        let mut symbols = BTreeSet::new();
        let continuity = ranking_continuity(items.iter().map(|item| item.rank))?;
        for item in &items {
            if !symbols.insert(item.symbol.clone()) {
                return Err(MarketDataError::DuplicateInstrument);
            }
        }
        for (old, new) in items.iter().zip(items.iter().skip(1)) {
            let order = old.basis.number().cmp(&new.basis.number());
            if (kind == RankingKind::TopLosers && order.is_gt())
                || (kind != RankingKind::TopLosers && order.is_lt())
            {
                return Err(MarketDataError::InvalidRank);
            }
        }
        Ok(Self {
            kind,
            market,
            duration,
            limit,
            items,
            observation: MarketObservation::from_response(ranked_at, collected)?,
            coverage: RankingCoverage::TopN,
            continuity,
            caution_filter,
        })
    }
    #[must_use]
    pub const fn continuity(&self) -> RankingContinuity {
        self.continuity
    }
    #[must_use]
    pub const fn caution_filter(&self) -> RankingCautionFilter {
        self.caution_filter
    }
    #[must_use]
    pub const fn kind(&self) -> RankingKind {
        self.kind
    }
    #[must_use]
    pub const fn market(&self) -> KnownMarket {
        self.market
    }
    #[must_use]
    pub const fn duration(&self) -> RankingDuration {
        self.duration
    }
    #[must_use]
    pub const fn limit(&self) -> RankingLimit {
        self.limit
    }
    #[must_use]
    pub const fn coverage(&self) -> RankingCoverage {
        self.coverage
    }
    #[must_use]
    pub fn items(&self) -> &[RankingItem] {
        &self.items
    }
    #[must_use]
    pub const fn observation(&self) -> MarketObservation {
        self.observation
    }
}
fn ranking_item(
    row: RankingRow,
    market: KnownMarket,
    unit: RankingUnit,
) -> Result<RankingItem, MarketDataError> {
    validate_market_symbol(&row.symbol, market)?;
    let currency = match market {
        KnownMarket::Kr => Currency::Krw,
        KnownMarket::Us => Currency::Usd,
    };
    if row.currency != currency {
        return Err(MarketDataError::InvalidCurrency);
    }
    let change_rate = ranking_change(row.change_rate.as_ref())?;
    let basis = match unit {
        RankingUnit::Shares => RankingBasis::Shares(
            Quantity::parse(&row.volume).or(Err(MarketDataError::InvalidNumber))?,
        ),
        RankingUnit::Money => RankingBasis::Amount(
            Money::parse(&row.amount, currency).or(Err(MarketDataError::InvalidNumber))?,
        ),
        RankingUnit::Ratio => match change_rate {
            RankingChange::Available(value) => RankingBasis::ChangeRate(value),
            RankingChange::Unavailable => return Err(MarketDataError::InvalidNumber),
        },
    };
    if basis.number().as_decimal() < rust_decimal::Decimal::ZERO && unit != RankingUnit::Ratio {
        return Err(MarketDataError::InvalidNumber);
    }
    Ok(RankingItem {
        rank: row.rank,
        symbol: row.symbol,
        basis,
        change_rate,
    })
}
pub(crate) fn validate_market_symbol(
    symbol: &Symbol,
    market: KnownMarket,
) -> Result<(), MarketDataError> {
    let text = symbol.as_str();
    if text.bytes().any(|byte| byte.is_ascii_lowercase()) {
        return Err(MarketDataError::InvalidSymbol);
    }
    match market {
        KnownMarket::Kr => {
            if text.len() != 6 || !text.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
                return Err(MarketDataError::InvalidSymbol);
            }
        }
        KnownMarket::Us => {
            if !text
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_alphabetic())
            {
                return Err(MarketDataError::InvalidSymbol);
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankingResponseInput {
    pub kind: MasterCode,
    pub market: KnownMarket,
    pub duration: MasterCode,
    pub limit: RankingLimit,
    pub rows: Vec<RankingRow>,
    pub ranked_at: Option<Timestamp>,
    pub collected: Timestamp,
    pub caution_filter: RankingCautionFilter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankingContinuity {
    Continuous,
    Gaps,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankingCautionFilter {
    Included,
    Excluded,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankingChange {
    Unavailable,
    Available(Ratio),
}
pub(crate) fn ranking_continuity(
    ranks: impl Iterator<Item = RankingOrdinal>,
) -> Result<RankingContinuity, MarketDataError> {
    let mut previous = RankingOrdinal::new(0);
    let mut continuity = RankingContinuity::Continuous;
    for (expected, rank) in (1..).zip(ranks) {
        if rank <= previous {
            return Err(MarketDataError::InvalidRank);
        }
        if rank != RankingOrdinal::new(expected) {
            continuity = RankingContinuity::Gaps;
        }
        previous = rank;
    }
    Ok(continuity)
}

pub(crate) fn ranking_change(
    input: Option<&NumericInput>,
) -> Result<RankingChange, MarketDataError> {
    match input {
        None => Ok(RankingChange::Unavailable),
        Some(raw) => {
            let value = Ratio::parse(raw).or(Err(MarketDataError::InvalidNumber))?;
            if value.as_decimal() < rust_decimal::Decimal::NEGATIVE_ONE {
                return Err(MarketDataError::InvalidNumber);
            }
            Ok(RankingChange::Available(value))
        }
    }
}
#[cfg(test)]
mod tests {
    use crate::{
        Currency, KnownMarket, MasterCode, Nanosecond, NumericInput, ObservationAlignment,
        OffsetSeconds, RankingCoverage, RankingDuration, RankingKind, RankingLimit, RankingOrdinal,
        RankingResponseInput, RankingRow, StockRanking, Symbol, Timestamp, UnixSeconds,
    };
    use proptest::test_runner::{Config, TestRunner};
    fn code(text: &str) -> MasterCode {
        MasterCode::try_new(text).expect("코드")
    }
    fn num(text: &str) -> NumericInput {
        NumericInput::try_new(text).expect("수치")
    }
    fn at() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(0),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn row(rank: usize) -> RankingRow {
        RankingRow {
            rank: RankingOrdinal::new(rank),
            symbol: Symbol::try_new(&format!("{rank:06}")).expect("종목"),
            currency: Currency::Krw,
            volume: num("100"),
            amount: num("200"),
            change_rate: Some(num("-0.1")),
        }
    }
    fn assert_context(value: &StockRanking) {
        assert_eq!(value.market(), KnownMarket::Kr);
        assert_eq!(value.duration(), RankingDuration::Week);
        assert_eq!(
            value.observation().alignment(),
            ObservationAlignment::Unknown
        );
    }
    #[test]
    fn ranking_permutations_are_identical_and_ranks_have_no_gaps() {
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(
                &(
                    0_usize..101,
                    proptest::collection::vec(proptest::num::u64::ANY, 100),
                ),
                |(count, keys)| {
                    let input = (1..=count).map(row).collect::<Vec<_>>();
                    let mut shuffled = input.clone();
                    shuffled.sort_by_key(|item| keys.get(*item.rank.as_usize()).copied());
                    let make = |rows| {
                        StockRanking::from_response(RankingResponseInput {
                            kind: code("MARKET_TRADING_VOLUME"),
                            market: KnownMarket::Kr,
                            duration: code("1d"),
                            limit: RankingLimit::try_new(100).expect("상한"),
                            rows,
                            ranked_at: None,
                            collected: at(),
                            caution_filter: crate::RankingCautionFilter::Included,
                        })
                        .expect("랭킹")
                    };
                    let expected = make(input);
                    let actual = make(shuffled);
                    proptest::prop_assert_eq!(&actual, &expected);
                    for (index, item) in actual.items().iter().enumerate() {
                        proptest::prop_assert_eq!(
                            *item.rank().as_usize(),
                            index.checked_add(1).expect("순위")
                        );
                    }
                    Ok(())
                },
            )
            .expect("4096회");
    }
    #[test]
    fn ranking_accepts_gaps_but_rejects_duplicate_ranks_and_wrong_basis_order() {
        let make = |rows| {
            StockRanking::from_response(RankingResponseInput {
                kind: code("MARKET_TRADING_AMOUNT"),
                market: KnownMarket::Kr,
                duration: code("1d"),
                limit: RankingLimit::try_new(2).expect("상한"),
                rows,
                ranked_at: None,
                collected: at(),
                caution_filter: crate::RankingCautionFilter::Included,
            })
        };
        make(vec![row(1), row(1)]).expect_err("거부");
        assert_eq!(
            make(vec![row(2)]).expect("빈자리").continuity(),
            crate::RankingContinuity::Gaps
        );
        assert_eq!(
            make(vec![row(1), row(3)]).expect("빈자리").continuity(),
            crate::RankingContinuity::Gaps
        );
        let mut high = row(2);
        high.amount = num("201");
        make(vec![row(1), high]).expect_err("거부");
        let mut duplicate = row(2);
        duplicate.symbol = row(1).symbol;
        make(vec![row(1), duplicate]).expect_err("거부");
        RankingLimit::try_new(0).expect_err("거부");
        RankingLimit::try_new(101).expect_err("거부");
    }
    #[test]
    fn ranking_kinds_units_periods_and_missing_time_are_preserved() {
        for (raw, kind, expected_unit) in [
            (
                "MARKET_TRADING_AMOUNT",
                RankingKind::MarketTradingAmount,
                crate::RankingUnit::Money,
            ),
            (
                "MARKET_TRADING_VOLUME",
                RankingKind::MarketTradingVolume,
                crate::RankingUnit::Shares,
            ),
            (
                "TOSS_SECURITIES_TRADING_AMOUNT",
                RankingKind::TossTradingAmount,
                crate::RankingUnit::Money,
            ),
            (
                "TOSS_SECURITIES_TRADING_VOLUME",
                RankingKind::TossTradingVolume,
                crate::RankingUnit::Shares,
            ),
            (
                "TOP_GAINERS",
                RankingKind::TopGainers,
                crate::RankingUnit::Ratio,
            ),
            (
                "TOP_LOSERS",
                RankingKind::TopLosers,
                crate::RankingUnit::Ratio,
            ),
        ] {
            let value = StockRanking::from_response(RankingResponseInput {
                kind: code(raw),
                market: KnownMarket::Kr,
                duration: code("1w"),
                limit: RankingLimit::try_new(1).expect("상한"),
                rows: vec![row(1)],
                ranked_at: None,
                collected: at(),
                caution_filter: crate::RankingCautionFilter::Included,
            })
            .expect("랭킹");
            assert_eq!(value.kind(), kind);
            assert_context(&value);
            assert_eq!(value.coverage(), RankingCoverage::TopN);
            assert_eq!(*value.limit().as_usize(), 1);
            assert_eq!(
                value.items().first().expect("행").symbol().as_str(),
                "000001"
            );
            assert_eq!(
                value.items().first().expect("행").basis().unit(),
                expected_unit
            );
        }
        assert_eq!(RankingKind::from_code(&code("NEW")), RankingKind::Unknown);
    }
    #[test]
    fn ranking_duration_catalog_and_all_failure_paths_are_explicit() {
        for (raw, expected) in [
            ("realtime", RankingDuration::Realtime),
            ("1d", RankingDuration::Day),
            ("1w", RankingDuration::Week),
            ("1mo", RankingDuration::Month),
            ("3mo", RankingDuration::ThreeMonths),
            ("6mo", RankingDuration::SixMonths),
            ("1y", RankingDuration::Year),
            ("new", RankingDuration::Unknown),
        ] {
            assert_eq!(RankingDuration::from_code(&code(raw)), expected);
        }
    }
    #[test]
    fn ranking_failure_paths_are_explicit() {
        let input = || RankingResponseInput {
            kind: code("MARKET_TRADING_AMOUNT"),
            market: KnownMarket::Kr,
            duration: code("1d"),
            limit: RankingLimit::try_new(1).expect("상한"),
            rows: vec![row(1)],
            ranked_at: None,
            collected: at(),
            caution_filter: crate::RankingCautionFilter::Included,
        };
        let mut unknown = input();
        unknown.kind = code("NEW");
        StockRanking::from_response(unknown).expect_err("미지 종류");
        let mut period = input();
        period.duration = code("NEW");
        StockRanking::from_response(period).expect_err("미지 기간");
        for kind in ["TOP_GAINERS", "TOP_LOSERS"] {
            let mut realtime = input();
            realtime.kind = code(kind);
            realtime.duration = code("realtime");
            StockRanking::from_response(realtime).expect_err("미지원 기간");
        }
        let mut count = input();
        count.rows.push(row(2));
        StockRanking::from_response(count).expect_err("초과");
        for amount in ["bad", "-1"] {
            let mut invalid = input();
            invalid.rows.first_mut().expect("행").amount = num(amount);
            StockRanking::from_response(invalid).expect_err("금액");
        }
        let mut currency = input();
        currency.rows.first_mut().expect("행").currency = Currency::Usd;
        StockRanking::from_response(currency).expect_err("통화");
        let mut symbol = input();
        symbol.rows.first_mut().expect("행").symbol = Symbol::try_new("AAPL").expect("심볼");
        StockRanking::from_response(symbol).expect_err("국내 심볼");
        let mut lower = input();
        lower.rows.first_mut().expect("행").symbol = Symbol::try_new("aapl").expect("심볼");
        StockRanking::from_response(lower).expect_err("소문자");
        let mut us = input();
        us.market = KnownMarket::Us;
        StockRanking::from_response(us.clone()).expect_err("미국 숫자 심볼");
        let first = us.rows.first_mut().expect("행");
        first.symbol = Symbol::try_new("AAPL").expect("심볼");
        first.currency = Currency::Usd;
        assert_eq!(
            StockRanking::from_response(us).expect("미국").market(),
            KnownMarket::Us
        );
    }
    #[test]
    fn losers_ascending_and_gainers_descending_are_checked() {
        let input = |kind| RankingResponseInput {
            kind: code(kind),
            market: KnownMarket::Kr,
            duration: code("1d"),
            limit: RankingLimit::try_new(2).expect("상한"),
            rows: vec![row(1), row(2)],
            ranked_at: Some(at()),
            collected: at(),
            caution_filter: crate::RankingCautionFilter::Included,
        };
        for (kind, rate) in [("TOP_LOSERS", "-0.2"), ("TOP_GAINERS", "0.2")] {
            let mut wrong = input(kind);
            wrong.rows.get_mut(1).expect("행").change_rate = Some(num(rate));
            StockRanking::from_response(wrong).expect_err("역순");
        }
        let mut valid = input("TOP_LOSERS");
        valid.rows.get_mut(1).expect("행").change_rate = Some(num("0.2"));
        StockRanking::from_response(valid).expect("오름차순");
    }

    #[test]
    fn ranking_invalid_volume_rate_and_future_clock_are_rejected() {
        let input = |kind| RankingResponseInput {
            kind: code(kind),
            market: KnownMarket::Kr,
            duration: code("1d"),
            limit: RankingLimit::try_new(1).expect("상한"),
            rows: vec![row(1)],
            ranked_at: None,
            collected: at(),
            caution_filter: crate::RankingCautionFilter::Included,
        };
        let mut volume = input("MARKET_TRADING_VOLUME");
        volume.rows.first_mut().expect("행").volume = num("bad");
        StockRanking::from_response(volume).expect_err("수량 문법");
        let mut rate = input("TOP_GAINERS");
        rate.rows.first_mut().expect("행").change_rate = Some(num("bad"));
        StockRanking::from_response(rate).expect_err("비율 문법");
        let mut future = input("TOP_GAINERS");
        future.ranked_at = Some(
            Timestamp::from_parts(
                UnixSeconds::new(3),
                Nanosecond::new(0),
                OffsetSeconds::new(0),
            )
            .expect("시각"),
        );
        StockRanking::from_response(future).expect_err("미래");
    }
}
#[cfg(test)]
mod review_tests {
    use super::{
        RankingCautionFilter, RankingChange, RankingContinuity, RankingKind, RankingLimit,
        RankingOrdinal, RankingResponseInput, RankingRow, StockRanking,
    };
    use crate::{
        Currency, KnownMarket, MasterCode, Nanosecond, NumericInput, OffsetSeconds, Symbol,
        Timestamp, UnixSeconds,
    };
    use proptest::test_runner::{Config, TestRunner};
    fn code(text: &str) -> MasterCode {
        MasterCode::try_new(text).expect("코드")
    }
    fn num(text: &str) -> NumericInput {
        NumericInput::try_new(text).expect("수치")
    }
    fn input() -> RankingResponseInput {
        RankingResponseInput {
            kind: code("MARKET_TRADING_AMOUNT"),
            market: KnownMarket::Kr,
            duration: code("1d"),
            limit: RankingLimit::try_new(100).expect("상한"),
            rows: vec![row(1)],
            ranked_at: None,
            collected: Timestamp::from_parts(
                UnixSeconds::new(0),
                Nanosecond::new(0),
                OffsetSeconds::new(0),
            )
            .expect("시각"),
            caution_filter: RankingCautionFilter::Included,
        }
    }
    fn row(rank: usize) -> RankingRow {
        RankingRow {
            rank: RankingOrdinal::new(rank),
            symbol: Symbol::try_new(&format!("{rank:06}")).expect("종목"),
            currency: Currency::Krw,
            volume: num("10"),
            amount: num("20"),
            change_rate: Some(num("0.0125")),
        }
    }
    #[test]
    fn gaps_zero_duplicates_and_filter_are_distinct() {
        let mut gaps = input();
        gaps.rows = vec![row(5), row(2)];
        gaps.caution_filter = RankingCautionFilter::Excluded;
        let fact = StockRanking::from_response(gaps).expect("빈자리 허용");
        assert_eq!(fact.continuity(), RankingContinuity::Gaps);
        assert_eq!(fact.caution_filter(), RankingCautionFilter::Excluded);
        assert_eq!(
            fact.items().first().expect("행").rank(),
            RankingOrdinal::new(2)
        );
        let normal = StockRanking::from_response(input()).expect("연속");
        assert_eq!(normal.continuity(), RankingContinuity::Continuous);
        assert_eq!(normal.caution_filter(), RankingCautionFilter::Included);
        for ranks in [vec![0], vec![1, 1]] {
            let mut bad = input();
            bad.rows = ranks.into_iter().map(row).collect();
            StockRanking::from_response(bad).expect_err("영점·동순위");
        }
    }
    #[test]
    fn equal_rank_with_different_symbols_is_rejected() {
        let mut duplicate = input();
        duplicate.rows = vec![row(1), row(2)];
        duplicate.rows.get_mut(1).expect("행").rank = RankingOrdinal::new(1);
        assert_eq!(
            StockRanking::from_response(duplicate),
            Err(crate::MarketDataError::InvalidRank)
        );
    }
    #[test]
    fn nullable_rates_and_decimal_fraction_are_preserved() {
        for kind in [
            "MARKET_TRADING_AMOUNT",
            "MARKET_TRADING_VOLUME",
            "TOSS_SECURITIES_TRADING_AMOUNT",
            "TOSS_SECURITIES_TRADING_VOLUME",
        ] {
            let mut missing = input();
            missing.kind = code(kind);
            missing.rows.first_mut().expect("행").change_rate = None;
            assert_eq!(
                StockRanking::from_response(missing)
                    .expect("등락률 없어도 유지")
                    .items()
                    .first()
                    .expect("행")
                    .change_rate(),
                RankingChange::Unavailable
            );
        }
        for kind in ["TOP_GAINERS", "TOP_LOSERS"] {
            let mut missing = input();
            missing.kind = code(kind);
            missing.rows.first_mut().expect("행").change_rate = None;
            StockRanking::from_response(missing).expect_err("기준 등락률 필수");
            let mut exact = input();
            exact.kind = code(kind);
            let fact = StockRanking::from_response(exact).expect("소수 비율");
            assert_eq!(fact.kind(), RankingKind::from_code(&code(kind)));
            let expected = crate::Ratio::parse(&num("0.0125")).expect("고정 소수 비율");
            assert_eq!(
                fact.items().first().expect("행").change_rate(),
                RankingChange::Available(expected)
            );
        }
        for rate in ["-1.0001", "-100"] {
            let mut bad = input();
            bad.rows.first_mut().expect("행").change_rate = Some(num(rate));
            StockRanking::from_response(bad).expect_err("비율 하한");
        }
        let mut boundary = input();
        boundary.rows.first_mut().expect("행").change_rate = Some(num("-1"));
        StockRanking::from_response(boundary).expect("전액 하락 경계");
    }
    #[test]
    fn us_symbol_first_character_is_the_only_invalid_field() {
        let mut bad = input();
        bad.market = KnownMarket::Us;
        let item = bad.rows.first_mut().expect("행");
        item.currency = Currency::Usd;
        item.symbol = Symbol::try_new("1ABC").expect("심볼 문법");
        StockRanking::from_response(bad).expect_err("미국 첫 글자");
    }
    #[test]
    fn arbitrary_gaps_are_strictly_ordered_and_input_permutations_normalize() {
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(
                &(
                    proptest::collection::btree_set(1_usize..1000, 0..101),
                    proptest::num::usize::ANY,
                ),
                |(ranks, seed)| {
                    let mut expected = input();
                    expected.rows = ranks.iter().copied().map(row).collect();
                    let continuity = if ranks
                        .iter()
                        .copied()
                        .zip(1..)
                        .all(|(rank, ordinal)| rank == ordinal)
                    {
                        RankingContinuity::Continuous
                    } else {
                        RankingContinuity::Gaps
                    };
                    let mut shuffled = expected.clone();
                    shuffled
                        .rows
                        .sort_by_key(|item| item.rank.as_usize().wrapping_mul(seed));
                    let normalized = StockRanking::from_response(shuffled).expect("정규화");
                    proptest::prop_assert_eq!(normalized.continuity(), continuity);
                    proptest::prop_assert_eq!(
                        &normalized,
                        &StockRanking::from_response(expected).expect("정렬 입력")
                    );
                    for (previous, current) in normalized
                        .items()
                        .iter()
                        .zip(normalized.items().iter().skip(1))
                    {
                        proptest::prop_assert!(previous.rank() < current.rank());
                    }
                    Ok(())
                },
            )
            .expect("4096회");
    }
}
