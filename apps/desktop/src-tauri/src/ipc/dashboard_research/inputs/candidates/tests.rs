use super::{both_markets, choice, evidence, headlines, ranked, readings};
use application::{
    CandlePriceBasis, CandleToolLine, CandlesToolView, CollectedEntry, MarketToolError,
    PricedCandlesView, PublicationTime, RankingToolKind, RankingToolLine, RankingsToolView,
};
use domain::{
    Currency, DisplayInstrumentName, KnownMarket, Money, Nanosecond, NewsItem, NewsSource,
    NormalizedUrl, NumericInput, OffsetSeconds, Symbol, Timestamp, UnixSeconds, UntrustedText,
};
use std::collections::BTreeSet;
use testkit_ports::{SteppedClock, tool_read_fakes::FixedMarketToolReads};

const NOW: i64 = 1_800_000_000;
fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
}
fn symbol(raw: &str) -> Symbol {
    Symbol::try_new(raw).expect("종목코드")
}
fn money(raw: &str) -> Money {
    Money::parse(&NumericInput::try_new(raw).expect("금액"), Currency::Usd).expect("달러")
}
fn list(tickers: &[&str], market: KnownMarket) -> RankingsToolView {
    RankingsToolView {
        market,
        kind: RankingToolKind::TradingAmount,
        collected_at: at(NOW),
        lines: tickers
            .iter()
            .map(|ticker| RankingToolLine {
                symbol: symbol(ticker),
                name: DisplayInstrumentName::try_new(ticker).expect("종목명"),
                metric: Some(money("900")),
                change_rate: None,
                price: None,
            })
            .collect(),
    }
}
fn day(time: i64, close: &str) -> CandleToolLine {
    let price = money(close);
    CandleToolLine {
        opened_at: at(time),
        open: price,
        high: price,
        low: price,
        close: price,
        volume: None,
    }
}
fn article(url: &str) -> CollectedEntry {
    CollectedEntry::News(NewsItem::new(
        NewsSource::Yonhap,
        NormalizedUrl::try_new(url).expect("URL"),
        UntrustedText::try_new("신규 설비 공시").expect("제목"),
        None,
    ))
}

#[test]
fn independent_market_failure_excludes_held_and_preserves_rank_budget() {
    let held = BTreeSet::from([symbol("HOLD")]);
    let us = ranked(
        Ok(Some(list(
            &["HOLD", "HOLD", "NEW", "NEW", "OTHER", "EXTRA"],
            KnownMarket::Us,
        ))),
        KnownMarket::Us,
        &held,
        at(NOW),
    );
    let (proposals, partial) = both_markets(
        ranked(
            Err(MarketToolError::Unavailable),
            KnownMarket::Kr,
            &held,
            at(NOW),
        ),
        us,
    )
    .expect("미국 후보");
    assert!(partial);
    assert_eq!(
        proposals
            .iter()
            .map(|prospect| (&prospect.target.symbol, prospect.position))
            .collect::<Vec<_>>(),
        vec![(&symbol("NEW"), 3), (&symbol("OTHER"), 5)]
    );
    assert!(both_markets(Err(()), Err(())).is_err());
    let (empty, failed) = both_markets(Ok(Vec::new()), Ok(Vec::new())).expect("빈 순위");
    assert!(empty.is_empty());
    assert!(!failed);
}

#[test]
fn live_candles_support_an_unheld_candidate_with_no_stored_articles() {
    let candidate = ranked(
        Ok(Some(list(&["NEW"], KnownMarket::Us))),
        KnownMarket::Us,
        &BTreeSet::new(),
        at(NOW),
    )
    .expect("순위")
    .pop()
    .expect("후보");
    let view = CandlesToolView {
        symbol: symbol("NEW"),
        market: KnownMarket::Us,
        collected_at: at(NOW),
        lines: vec![
            day(NOW - 60, "112"),
            day(NOW + 30, "999"),
            day(NOW - 86_400, "100"),
        ],
    };
    let market = FixedMarketToolReads::default()
        .with_candles(Ok(Some(view)))
        .with_candle_basis(CandlePriceBasis::Adjusted);
    let (prices, failed) = tauri::async_runtime::block_on(readings(
        &market,
        &candidate.target,
        &SteppedClock::new(at(NOW)),
    ));
    assert!(!failed);
    let (_, selected, evidence_rows) = choice(candidate, prices, Vec::new()).expect("새 종목");
    assert_eq!(
        selected,
        application::DashboardSubject::Candidate(
            symbol("NEW"),
            DisplayInstrumentName::try_new("NEW").expect("이름")
        )
    );
    assert_eq!(evidence_rows.len(), 2);
    let text = evidence_rows.last().expect("근거").title.as_untrusted_str();
    assert!(text.contains("100 달러") && text.contains("112 달러") && text.contains("수정"));
    assert!(!text.contains("999"));
}

#[test]
fn ranking_alone_and_another_symbols_or_future_bars_are_not_confirmation() {
    let prospect = ranked(
        Ok(Some(list(&["NEW"], KnownMarket::Us))),
        KnownMarket::Us,
        &BTreeSet::new(),
        at(NOW),
    )
    .expect("순위")
    .pop()
    .expect("후보");
    let mut priced = PricedCandlesView {
        basis: CandlePriceBasis::Unadjusted,
        view: CandlesToolView {
            symbol: symbol("WRONG"),
            market: KnownMarket::Us,
            collected_at: at(NOW),
            lines: vec![day(NOW - 60, "110")],
        },
    };
    assert!(evidence::candles(&priced, &prospect.target, at(NOW)).is_none());
    priced.view.symbol = symbol("NEW");
    priced.view.collected_at = at(NOW + 60);
    assert!(evidence::candles(&priced, &prospect.target, at(NOW)).is_none());
    assert!(choice(prospect, Vec::new(), Vec::new()).is_none());
}

#[test]
fn original_publication_and_observation_have_separate_dates_and_only_public_verified_urls() {
    let news = article("https://www.yna.co.kr/view/ABC");
    let source = headlines::article(
        &news,
        PublicationTime::Known(at(NOW - 86_400)),
        at(NOW - 60),
        at(NOW),
    )
    .expect("보도");
    assert_eq!(source.observed_at, at(NOW - 60));
    assert!(
        source
            .title
            .as_untrusted_str()
            .contains(&crate::ipc::briefing::kst_iso(at(NOW - 86_400)).expect("발행일"))
    );
    assert!(source.title.as_untrusted_str().contains("www.yna.co.kr"));
    for (published, collected) in [
        (PublicationTime::NotProvided, NOW),
        (PublicationTime::Known(at(NOW - 604_801)), NOW),
        (PublicationTime::Known(at(NOW + 1)), NOW),
        (PublicationTime::Known(at(NOW)), NOW + 60),
    ] {
        assert!(headlines::article(&news, published, at(collected), at(NOW)).is_none());
    }
    for url in ["https://localhost/a", "https://127.0.0.1/a"] {
        assert_eq!(
            NormalizedUrl::try_new(url),
            Err(domain::UrlError::InvalidFormat)
        );
    }
    for url in ["http://www.yna.co.kr/a", "https://internal.local/a"] {
        assert!(
            headlines::article(
                &article(url),
                PublicationTime::Known(at(NOW - 60)),
                at(NOW),
                at(NOW)
            )
            .is_none()
        );
    }
}

#[test]
fn disclosure_uses_its_filing_time_and_public_receipt_and_flows_ignore_future_sessions() {
    let receipt =
        domain::ReceiptNo::Dart(domain::DartReceiptNo::try_new("20261125000001").expect("접수"));
    let url = NormalizedUrl::try_new("https://dart.fss.or.kr/dsaf001/main.do?rcpNo=20261125000001")
        .expect("공개 주소");
    let entry = CollectedEntry::Disclosure(domain::Disclosure::new(
        receipt,
        url.clone(),
        UntrustedText::try_new("시설 투자 공시").expect("제목"),
        None,
    ));
    let source = headlines::article(
        &entry,
        PublicationTime::Known(at(NOW - 120)),
        at(NOW - 60),
        at(NOW),
    )
    .expect("공시");
    assert!(source.title.as_untrusted_str().contains("공시 제출"));
    assert_eq!(source.url, Some(url));
    assert_eq!(source.observed_at, at(NOW - 60));
    let prospect = ranked(
        Ok(Some(list(&["123456"], KnownMarket::Kr))),
        KnownMarket::Kr,
        &BTreeSet::new(),
        at(NOW),
    )
    .expect("순위")
    .pop()
    .expect("한국 후보");
    let line = |date, value| application::InvestorFlowToolLine {
        trading_date: at(date),
        foreign_net: Some(
            Money::parse(&NumericInput::try_new(value).expect("금액"), Currency::Krw)
                .expect("원화"),
        ),
        institution_net: None,
        individual_net: None,
        short_selling_ratio: None,
    };
    let flow = application::InvestorFlowToolView {
        symbol: symbol("123456"),
        market: KnownMarket::Kr,
        collected_at: at(NOW),
        lines: vec![line(NOW - 60, "125"), line(NOW + 60, "999")],
    };
    let supply = evidence::flows(&flow, &prospect.target, at(NOW)).expect("완료된 수급");
    assert!(supply.title.as_untrusted_str().contains("125 원"));
    assert!(!supply.title.as_untrusted_str().contains("999"));
}
