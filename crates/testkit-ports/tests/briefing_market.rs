#[cfg(test)]
mod tests {
    use application::{
        BriefingContext, BriefingContextSource, MarketBriefingContext, MarketOverviewToolView,
        MarketToolError, OverviewItem, OverviewToolLine, OverviewUnit, SessionStatus,
        SessionToolLine, SystemPortError, ValueProvenance, market_briefing_facts,
    };
    use domain::{
        BriefingKind, KnownMarket, Nanosecond, NumericInput, OffsetSeconds, Ratio, Timestamp,
        UnixSeconds, UntrustedText,
    };
    use std::future::Future;
    use std::task::{Context, Poll, Waker};
    use testkit_ports::tool_read_fakes::FixedMarketToolReads;

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("즉시 완료"),
        }
    }

    fn at() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(1_795_995_600),
            Nanosecond::new(0),
            OffsetSeconds::new(32_400),
        )
        .expect("시각")
    }

    fn ratio(text: &str) -> Ratio {
        Ratio::parse(&NumericInput::try_new(text).expect("입력")).expect("비율")
    }

    fn line(item: OverviewItem, unit: OverviewUnit, value: &str, change: &str) -> OverviewToolLine {
        OverviewToolLine {
            item,
            unit,
            value: ratio(value),
            change_rate: Some(ratio(change)),
            provenance: match item {
                OverviewItem::Sp500 | OverviewItem::Nasdaq100 => ValueProvenance::Approximate,
                OverviewItem::Kospi
                | OverviewItem::Kosdaq
                | OverviewItem::KrBond3Y
                | OverviewItem::KrBond10Y
                | OverviewItem::UsdKrw
                | OverviewItem::UsTreasury2Y
                | OverviewItem::UsTreasury10Y
                | OverviewItem::CrudeOil
                | OverviewItem::Gold
                | OverviewItem::DollarIndex => ValueProvenance::Official,
            },
            collected_at: at(),
        }
    }

    fn overview() -> MarketOverviewToolView {
        MarketOverviewToolView {
            lines: vec![
                line(
                    OverviewItem::Kospi,
                    OverviewUnit::IndexPoints,
                    "2650.3",
                    "0.0112",
                ),
                line(
                    OverviewItem::KrBond3Y,
                    OverviewUnit::YieldPercent,
                    "2.81",
                    "-0.004",
                ),
                line(OverviewItem::UsdKrw, OverviewUnit::KrwPerUsd, "1385.2", "0"),
                line(
                    OverviewItem::Sp500,
                    OverviewUnit::UsdPrice,
                    "598.4",
                    "-0.0031",
                ),
            ],
            sessions: vec![
                SessionToolLine {
                    market: KnownMarket::Kr,
                    status: SessionStatus::Holiday,
                    collected_at: at(),
                },
                SessionToolLine {
                    market: KnownMarket::Us,
                    status: SessionStatus::Closed,
                    collected_at: at(),
                },
            ],
        }
    }

    fn texts(facts: &[UntrustedText]) -> Vec<&str> {
        facts.iter().map(UntrustedText::as_untrusted_str).collect()
    }

    #[test]
    fn morning_facts_quote_every_market_line_with_units_signs_and_the_etf_marker() {
        let facts = market_briefing_facts(BriefingKind::Morning, &overview());
        assert_eq!(
            texts(&facts),
            vec![
                "시장: 코스피 2650.30 (+1.12%)",
                "시장: 국고채 3년 2.81% (-0.40%)",
                "시장: 원/달러 1385.20원 (0.00%)",
                "시장: 미국 S&P500 $598.40 (-0.31%) [ETF 근사값]",
                "오늘 일정: 한국장 휴장",
                "오늘 일정: 미국장 장 마감",
            ]
        );
    }

    #[test]
    fn pre_us_facts_keep_only_us_lines_and_the_us_session() {
        let facts = market_briefing_facts(BriefingKind::PreUsMarket, &overview());
        assert_eq!(
            texts(&facts),
            vec![
                "시장: 원/달러 1385.20원 (0.00%)",
                "시장: 미국 S&P500 $598.40 (-0.31%) [ETF 근사값]",
                "오늘 일정: 미국장 장 마감",
            ]
        );
        assert_eq!(
            market_briefing_facts(BriefingKind::ClosingReview, &overview()),
            Vec::new()
        );
    }

    struct Inner;

    impl BriefingContextSource for Inner {
        fn context(
            &self,
            _kind: BriefingKind,
            _now: Timestamp,
        ) -> impl Future<Output = Result<BriefingContext, SystemPortError>> + Send {
            std::future::ready(Ok(BriefingContext {
                instruments: Vec::new(),
                evidence: Vec::new(),
                code_facts: vec![UntrustedText::try_new("기존 사실").expect("글")],
            }))
        }
    }

    #[test]
    fn closing_review_keeps_the_holdings_context_without_fetching_market_data() {
        let market = FixedMarketToolReads::default().with_overview(Ok(Some(overview())));
        let context = run(MarketBriefingContext {
            inner: &Inner,
            market: &market,
        }
        .context(BriefingKind::ClosingReview, at()))
        .expect("문맥");
        assert_eq!(texts(&context.code_facts), vec!["기존 사실"]);
        assert_eq!(market.calls(), []);
    }

    #[test]
    fn the_wrapper_appends_market_facts_and_stays_silent_when_reads_fail() {
        let connected = FixedMarketToolReads::default().with_overview(Ok(Some(overview())));
        let context = run(MarketBriefingContext {
            inner: &Inner,
            market: &connected,
        }
        .context(BriefingKind::Morning, at()))
        .expect("문맥");
        assert_eq!(context.code_facts.len(), 7);
        assert_eq!(context.code_facts[0].as_untrusted_str(), "기존 사실");
        let failing = FixedMarketToolReads::default().failing_with(MarketToolError::NotConnected);
        let silent = run(MarketBriefingContext {
            inner: &Inner,
            market: &failing,
        }
        .context(BriefingKind::Morning, at()))
        .expect("문맥");
        assert_eq!(texts(&silent.code_facts), vec!["기존 사실"]);
    }
}
