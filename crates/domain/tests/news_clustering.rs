#[cfg(test)]
mod tests {
    use domain::{
        ClusterDecision, DomainEvent, InstrumentId, Nanosecond, NewsClusterBook, NewsItem,
        NewsSource, NormalizedUrl, OffsetSeconds, TimedNews, Timestamp, TitleBigrams,
        TitleSimilarity, UnixSeconds, UntrustedText, dedupe_by_url,
    };

    const HOUR: i64 = 3_600;

    fn text(raw: &str) -> UntrustedText {
        UntrustedText::try_new(raw).expect("제목")
    }

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(1_800_000_000_i64.checked_add(seconds).expect("시각 범위")),
            Nanosecond::new(0),
            OffsetSeconds::new(32_400),
        )
        .expect("시각")
    }

    fn news(url: &str, title: &str) -> NewsItem {
        NewsItem::new(
            NewsSource::Hankyung,
            NormalizedUrl::try_new(url).expect("주소"),
            UntrustedText::try_new(title).expect("제목"),
            None,
        )
    }

    fn timed(url: &str, title: &str, seconds: i64) -> TimedNews {
        TimedNews::new(news(url, title), at(seconds))
    }

    fn instrument(raw: &str) -> InstrumentId {
        InstrumentId::try_new(raw).expect("종목")
    }

    #[test]
    fn identical_normalized_url_is_the_same_item() {
        let items = vec![
            news(
                "https://www.mk.co.kr/news/1?utm_source=rss",
                "삼성전자 실적",
            ),
            news("https://www.mk.co.kr/news/1#top", "삼성전자 실적 (수정)"),
            news("https://www.mk.co.kr/news/2", "삼성전자 실적"),
        ];
        assert_eq!(dedupe_by_url(items).len(), 2_usize);
        let samsung = instrument("KR7005930003");
        let mut book = NewsClusterBook::new();
        assert_eq!(
            book.admit(
                &samsung,
                timed(
                    "https://www.mk.co.kr/news/1",
                    "삼성전자 3분기 영업익 10조",
                    0
                )
            ),
            ClusterDecision::Started(samsung.clone())
        );
        assert_eq!(
            book.admit(
                &samsung,
                timed(
                    "https://www.mk.co.kr/news/1?utm_campaign=x",
                    "전혀 다른 제목",
                    HOUR
                )
            ),
            ClusterDecision::DuplicateUrl
        );
    }

    #[test]
    fn similar_titles_for_the_same_stock_within_48_hours_join_and_notify_by_stock_key() {
        let samsung = instrument("KR7005930003");
        let mut book = NewsClusterBook::new();
        let _started = book.admit(
            &samsung,
            timed(
                "https://a.example.com/1",
                "삼성전자 3분기 영업이익 10조 돌파",
                0,
            ),
        );
        let joined = book.admit(
            &samsung,
            timed(
                "https://b.example.com/2",
                "삼성전자, 3분기 영업이익 10조 돌파했다",
                169_200,
            ),
        );
        assert_eq!(joined, ClusterDecision::Joined(samsung.clone()));
        assert_eq!(
            joined.event(),
            Some(DomainEvent::NewsClusterGrew(samsung.clone()))
        );
        assert_eq!(ClusterDecision::Started(samsung.clone()).event(), None);
        assert_eq!(ClusterDecision::DuplicateUrl.event(), None);
        let late = book.admit(
            &samsung,
            timed(
                "https://c.example.com/3",
                "삼성전자 3분기 영업이익 10조 돌파",
                176_400,
            ),
        );
        assert_eq!(late, ClusterDecision::Started(samsung.clone()));
        let clusters: Vec<_> = book.clusters_for(&samsung).collect();
        assert_eq!(clusters.len(), 2_usize);
        assert_eq!(clusters[0].members().len(), 1_usize);
    }

    #[test]
    fn other_company_articles_are_never_mixed_into_the_cluster() {
        let samsung = instrument("KR7005930003");
        let hynix = instrument("KR7000660001");
        let mut book = NewsClusterBook::new();
        let _started = book.admit(
            &samsung,
            timed(
                "https://a.example.com/1",
                "삼성전자 반도체 실적 개선 전망",
                0,
            ),
        );
        assert_eq!(
            book.admit(
                &hynix,
                timed(
                    "https://a.example.com/2",
                    "삼성전자 반도체 실적 개선 전망",
                    HOUR
                )
            ),
            ClusterDecision::Started(hynix.clone())
        );
        assert_eq!(
            book.admit(
                &samsung,
                timed(
                    "https://a.example.com/3",
                    "SK하이닉스 반도체 실적 개선 전망",
                    HOUR
                )
            ),
            ClusterDecision::Started(samsung.clone())
        );
    }

    #[test]
    fn comparison_is_with_the_representative_not_single_linkage() {
        let samsung = instrument("KR7005930003");
        let mut book = NewsClusterBook::new();
        let _rep = book.admit(
            &samsung,
            timed("https://x.example.com/1", "가나다라마바사아", 0),
        );
        assert_eq!(
            book.admit(
                &samsung,
                timed("https://x.example.com/2", "가나다라마바자차", HOUR)
            ),
            ClusterDecision::Joined(samsung.clone())
        );
        assert_eq!(
            book.admit(
                &samsung,
                timed("https://x.example.com/3", "마바자차카타파하", HOUR)
            ),
            ClusterDecision::Started(samsung.clone())
        );
    }

    #[test]
    fn jaccard_threshold_is_exactly_forty_five_percent() {
        let base = TitleBigrams::from_title(&text("abcdefghijk"));
        assert_eq!(
            base.similarity(&TitleBigrams::from_title(&text("abcdefghijk"))),
            TitleSimilarity::Similar
        );
        let nine_of_twenty = TitleBigrams::from_title(&text("abcdefghij012345678z"));
        assert_eq!(
            TitleBigrams::from_title(&text("abcdefghijk")).similarity(&nine_of_twenty),
            TitleSimilarity::Similar
        );
        let lower = TitleBigrams::from_title(&text("abcdefghij0123456789z"));
        assert_eq!(
            TitleBigrams::from_title(&text("abcdefghijk")).similarity(&lower),
            TitleSimilarity::Distinct
        );
        assert_eq!(
            TitleBigrams::from_title(&text("!")).similarity(&TitleBigrams::from_title(&text("?"))),
            TitleSimilarity::Distinct
        );
    }
}
