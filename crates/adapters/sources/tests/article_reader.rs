#[cfg(test)]
mod tests {
    use std::future::{Future, ready};
    use std::pin::Pin;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    use std::time::Duration;

    use application::Clock;
    use domain::{
        AiBodyReadToggle, CollectedItem, Nanosecond, NewsItem, NewsSource, NormalizedUrl,
        OffsetSeconds, SourceAiBodyReadSettings, Timestamp, UnixSeconds, UntrustedText,
        prepare_news_for_ai,
    };
    use sources::{ArticleReadError, ArticleReader, SourceEndpoint, SourceHttpClient, SourceWait};
    use testkit::{ConnectionObservation, ConnectionProbe, serve_sequence};

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

    fn client(port: u16) -> Arc<SourceHttpClient> {
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

    #[test]
    fn disabled_locked_web_and_off_domain_articles_make_zero_connections() {
        let rt = runtime();
        let probe = ConnectionProbe::bind().expect("probe");
        let port = probe.port().expect("port");
        let reader = ArticleReader::new(client(port));
        let mut settings = SourceAiBodyReadSettings::all_disabled();

        let hk_item = NewsItem::new(
            NewsSource::Hankyung,
            NormalizedUrl::try_new("https://www.hankyung.com/article/202611110253i").expect("url"),
            UntrustedText::try_new("제목").expect("title"),
            Some(UntrustedText::try_new("요약").expect("summary")),
        );
        assert_eq!(
            rt.block_on(reader.read_transient(hk_item.collected(), &settings)),
            Err(ArticleReadError::AiBodyReadDisabled(NewsSource::Hankyung))
        );

        let yna_item = NewsItem::new(
            NewsSource::Yonhap,
            NormalizedUrl::try_new("https://www.yna.co.kr/view/AKR1").expect("url"),
            UntrustedText::try_new("연합 제목").expect("title"),
            Some(UntrustedText::try_new("연합 요약").expect("summary")),
        );
        assert_eq!(
            rt.block_on(reader.read_transient(yna_item.collected(), &settings)),
            Err(ArticleReadError::SourceLockedOff(NewsSource::Yonhap))
        );

        let web_item = CollectedItem::from_web(
            NormalizedUrl::try_new("https://www.hankyung.com/article/web").expect("url"),
            UntrustedText::try_new("웹 제목").expect("title"),
            None,
        );
        assert_eq!(
            rt.block_on(reader.read_transient(&web_item, &settings)),
            Err(ArticleReadError::WebSourceDisallowed)
        );

        settings
            .set(&NewsSource::Hankyung, AiBodyReadToggle::Enabled)
            .expect("enable");
        let cross_host = NewsItem::new(
            NewsSource::Hankyung,
            NormalizedUrl::try_new("https://evil.example.com/article/1").expect("url"),
            UntrustedText::try_new("외부 호스트").expect("title"),
            None,
        );
        assert_eq!(
            rt.block_on(reader.read_transient(cross_host.collected(), &settings)),
            Err(ArticleReadError::DisallowedArticleHost)
        );
        assert_eq!(
            probe.observe().expect("probe"),
            ConnectionObservation::NoConnection
        );
    }

    #[test]
    fn enabled_press_source_reads_transient_body_and_feeds_ai_input() {
        let rt = runtime();
        let html =
            "<html><body><article><p>반도체 업황이 개선되고 있다.</p></article></body></html>";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n{html}",
            html.len()
        )
        .into_bytes();
        let server = serve_sequence(vec![response]).expect("server");
        let reader = ArticleReader::new(client(server.port()));
        let mut settings = SourceAiBodyReadSettings::all_disabled();
        settings
            .set(&NewsSource::Hankyung, AiBodyReadToggle::Enabled)
            .expect("enable");

        let item = NewsItem::new(
            NewsSource::Hankyung,
            NormalizedUrl::try_new("https://www.hankyung.com/article/202611110253i").expect("url"),
            UntrustedText::try_new("반도체 전망").expect("title"),
            Some(UntrustedText::try_new("짧은 요약").expect("summary")),
        );
        let body = rt
            .block_on(reader.read_transient(item.collected(), &settings))
            .expect("read");
        assert_eq!(body.as_untrusted_str(), "반도체 업황이 개선되고 있다.");
        let ai_input = prepare_news_for_ai(&item, Some(&body), &settings);
        assert_eq!(
            ai_input.body().expect("body").as_untrusted_str(),
            "반도체 업황이 개선되고 있다."
        );
        let _captured = server.finish().expect("captured");
    }
}
