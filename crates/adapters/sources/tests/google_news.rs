#[cfg(test)]
mod tests {
    use std::future::{Future, ready};
    use std::pin::Pin;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    use std::time::Duration;

    use application::{
        AiReadPolicy, Clock, CollectedEntry, CollectedObservation, ConditionalGet,
        DataAvailability, FeedCursor, FeedSource, NewsFeed, PublicationTime, Retention,
        UserAgentRequirement,
    };
    use domain::{
        AiBodyReadSettingError, AiBodyReadToggle, ApprovedWebSearchQuery, DataSource, Nanosecond,
        NewsItem, NewsSource, NormalizedUrl, OffsetSeconds, SourceAiBodyReadSettings, SpanSeconds,
        TimeSpan, Timestamp, UnixSeconds, UntrustedText, WebSearchCandidateQuery,
        WebSearchInspection, WebSearchSensitiveContext, inspect_web_search_query,
        prepare_news_for_ai,
    };
    use sources::{
        GOOGLE_NEWS_RETENTION_SECONDS, GoogleNewsFeed, GoogleNewsRetentionStatus, SourceEndpoint,
        SourceHttpClient, SourceRegistry, SourceWait, google_news_retention_status,
        prune_expired_google_news,
    };
    use testkit::serve_sequence;

    struct AutoClock(AtomicU64);

    impl Clock for AutoClock {
        fn now(&self) -> Timestamp {
            let secs = 1_800_000_000_i64.saturating_add(
                i64::try_from(
                    self.0
                        .load(Ordering::SeqCst)
                        .checked_div(1_000_000_000)
                        .expect("div"),
                )
                .expect("sec"),
            );
            Timestamp::from_parts(
                UnixSeconds::new(secs),
                Nanosecond::new(0),
                OffsetSeconds::new(0),
            )
            .expect("ts")
        }
    }

    impl SourceWait for AutoClock {
        fn wait(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            let delta = u64::try_from(duration.as_nanos()).expect("nanos");
            self.0.fetch_add(delta, Ordering::SeqCst);
            Box::pin(ready(()))
        }
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
    }

    fn make_client(port: u16) -> Arc<SourceHttpClient> {
        let clock = Arc::new(AutoClock(AtomicU64::new(0)));
        let clock_copy = Arc::clone(&clock);
        Arc::new(
            SourceHttpClient::new(
                SourceEndpoint::local(port).expect("endpoint"),
                clock_copy,
                clock,
                None,
            )
            .expect("client"),
        )
    }

    fn approved_query(raw: &str) -> ApprovedWebSearchQuery {
        let candidate = WebSearchCandidateQuery::new(raw);
        let context = WebSearchSensitiveContext::default();
        match inspect_web_search_query(&candidate, &context) {
            WebSearchInspection::Approved(ok) => ok,
            WebSearchInspection::Rejected(rejected) => {
                panic!("expected approved query, got {rejected:?}")
            }
        }
    }

    fn ts(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("ts")
    }

    fn xml_response(body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/xml; charset=utf-8\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    const GOOGLE_NEWS_RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0"><channel><title>"삼성전자" - Google 뉴스</title>
<copyright>made available solely for the purpose of rendering Google News results within a personal feed reader for personal, non-commercial use.</copyright>
<item>
  <title>삼성전자 HBM 공급 확대 기대 - 한국경제</title>
  <link>https://news.google.com/rss/articles/CBMiT0FVX3lxTE1?oc=5</link>
  <pubDate>Wed, 11 Nov 2026 07:30:00 GMT</pubDate>
  <description><![CDATA[<a href="https://news.google.com/rss/articles/CBMiT0FVX3lxTE1">기사 요약 HTML 스니펫은 버려져야 한다</a>]]></description>
  <source url="https://www.hankyung.com">한국경제</source>
</item>
<item>
  <title>삼성전자 반도체 투자 가속 - 매일경제</title>
  <link>https://news.google.com/rss/articles/CBMiT0FVX3lxTE2?oc=5</link>
  <pubDate>Wed, 11 Nov 2026 08:00:00 GMT</pubDate>
  <description><![CDATA[<p>절대 AI나 표시에 들어가면 안 되는 본문 요약</p>]]></description>
  <source url="https://www.mk.co.kr">매일경제</source>
</item>
</channel></rss>"#;

    fn assert_google_metadata(feed: &GoogleNewsFeed) {
        assert_eq!(feed.source(), FeedSource::News(NewsSource::GoogleNews));
        let meta = feed.metadata();
        assert_eq!(
            meta.retention(),
            Retention::For(
                TimeSpan::try_new(
                    SpanSeconds::new(GOOGLE_NEWS_RETENTION_SECONDS),
                    Nanosecond::new(0),
                )
                .expect("span")
            )
        );
        assert_eq!(meta.allow_ai_read(), AiReadPolicy::TitleOnly);
        assert_eq!(meta.supports_conditional_get(), ConditionalGet::Unsupported);
        assert_eq!(meta.requires_user_agent(), UserAgentRequirement::Standard);
    }

    #[test]
    fn google_news_strips_descriptions_keeps_title_and_link_and_sends_title_only_to_ai() {
        let server = serve_sequence(vec![
            xml_response(GOOGLE_NEWS_RSS),
            xml_response(GOOGLE_NEWS_RSS),
        ])
        .expect("server");
        let feed = GoogleNewsFeed::new(make_client(server.port()), approved_query("삼성전자"));
        assert_google_metadata(&feed);

        let first = runtime()
            .block_on(feed.new_items(FeedCursor::Start))
            .expect("first");
        let (items, cursor) = match first {
            DataAvailability::Available(pair) => pair,
            other @ (DataAvailability::NotConfigured | DataAvailability::NoData) => {
                panic!("expected items, got {other:?}")
            }
        };
        assert_eq!(items.len(), 2_usize);

        let mut settings = SourceAiBodyReadSettings::all_disabled();
        assert_eq!(
            settings.set(&NewsSource::GoogleNews, AiBodyReadToggle::Enabled),
            Err(AiBodyReadSettingError::SourceLockedOff(
                NewsSource::GoogleNews
            ))
        );
        let extra = UntrustedText::try_new("임시 본문").expect("body");
        for obs in &items {
            let CollectedEntry::News(news_item) = obs.entry() else {
                panic!("expected news entry");
            };
            assert_eq!(news_item.collected().body(), None);
            let ai_input = prepare_news_for_ai(news_item, Some(&extra), &settings);
            assert_eq!(ai_input.title(), news_item.collected().title());
            assert_eq!(ai_input.summary(), None);
            assert_eq!(ai_input.body(), None);
        }

        assert_eq!(
            runtime().block_on(feed.new_items(cursor)).expect("second"),
            DataAvailability::NoData
        );

        let wire_calls = server.finish().expect("finish");
        let wire_head = std::str::from_utf8(&wire_calls[0]).expect("utf8");
        assert!(wire_head.contains("/rss/search?q="), "{wire_head}");
        assert!(
            wire_head.contains("hl=ko&gl=KR&ceid=KR%3Ako"),
            "{wire_head}"
        );
    }

    #[test]
    fn google_news_display_items_expose_only_title_link_and_publisher() {
        let server = serve_sequence(vec![xml_response(GOOGLE_NEWS_RSS)]).expect("server");
        let feed = GoogleNewsFeed::new(make_client(server.port()), approved_query("삼성전자"));
        let display_items = runtime()
            .block_on(feed.fetch_display_items())
            .expect("display");
        assert_eq!(display_items.len(), 2_usize);
        assert_eq!(
            display_items[0].title().as_untrusted_str(),
            "삼성전자 HBM 공급 확대 기대 - 한국경제"
        );
        assert_eq!(
            display_items[0].link().as_str(),
            "https://news.google.com/rss/articles/CBMiT0FVX3lxTE1?oc=5"
        );
        assert_eq!(
            display_items[0]
                .publisher()
                .map(UntrustedText::as_untrusted_str),
            Some("한국경제")
        );
        let _calls = server.finish().expect("finish");
    }

    #[test]
    fn google_news_items_expire_at_exact_seven_day_boundary_while_other_sources_remain() {
        let collected = ts(1_800_000_000);
        let just_before = ts(1_800_000_000_i64.saturating_add(604_799_i64));
        let exact_seven_days = ts(1_800_000_000_i64.saturating_add(604_800_i64));
        let after_eight_days = ts(1_800_000_000_i64.saturating_add(691_200_i64));

        assert_eq!(
            google_news_retention_status(collected, just_before),
            GoogleNewsRetentionStatus::Keep
        );
        assert_eq!(
            google_news_retention_status(collected, exact_seven_days),
            GoogleNewsRetentionStatus::Expired
        );
        assert_eq!(
            google_news_retention_status(collected, after_eight_days),
            GoogleNewsRetentionStatus::Expired
        );

        let old_google = CollectedObservation::new(
            CollectedEntry::News(NewsItem::new(
                NewsSource::GoogleNews,
                NormalizedUrl::try_new("https://news.google.com/rss/articles/old").expect("u"),
                UntrustedText::try_new("7일 지난 구글 기사").expect("t"),
                None,
            )),
            PublicationTime::NotProvided,
            collected,
        );
        let fresh_google = CollectedObservation::new(
            CollectedEntry::News(NewsItem::new(
                NewsSource::GoogleNews,
                NormalizedUrl::try_new("https://news.google.com/rss/articles/new").expect("u"),
                UntrustedText::try_new("최근 구글 기사").expect("t"),
                None,
            )),
            PublicationTime::NotProvided,
            just_before,
        );
        let old_hankyung = CollectedObservation::new(
            CollectedEntry::News(NewsItem::new(
                NewsSource::Hankyung,
                NormalizedUrl::try_new("https://www.hankyung.com/article/old").expect("u"),
                UntrustedText::try_new("한경 기사").expect("t"),
                None,
            )),
            PublicationTime::NotProvided,
            collected,
        );

        let pruned = prune_expired_google_news(
            vec![old_google, fresh_google.clone(), old_hankyung.clone()],
            exact_seven_days,
        );
        assert_eq!(pruned.deleted_count, 1_usize);
        assert_eq!(pruned.retained, vec![fresh_google, old_hankyung]);
    }

    #[test]
    fn naver_search_api_is_absent_from_data_sources_and_registry_hosts() {
        for source in DataSource::all() {
            let name = format!("{source:?}").to_ascii_lowercase();
            assert!(!name.contains("naver"), "{name}");
        }
        for spec in SourceRegistry::all() {
            for host in spec.allowed_hosts() {
                let lower = host.to_ascii_lowercase();
                assert!(!lower.contains("naver"), "{host}");
                assert!(!lower.contains("ntruss"), "{host}");
            }
        }
    }
}
