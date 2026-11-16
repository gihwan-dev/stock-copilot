#[cfg(test)]
mod tests {
    use application::{
        AiReadPolicy, BrokerCapabilities, BrokerCapability, CapabilitySupport, ConditionalGet,
        DataAvailability, FeedCursor, FeedSource, InformationError, MacroSeriesId, NewsMetadata,
        Notification, ResetTime, Retention, UserAgentRequirement,
    };
    use domain::{Nanosecond, NewsSource, OffsetSeconds, Symbol, Timestamp, UnixSeconds};
    use std::{
        collections::BTreeSet,
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{
        contracts,
        scripted::{
            FundamentalsCall, MacroSeriesCall, NewsFeedCall, NotifierCall, ScriptedFundamentals,
            ScriptedMacroSeries, ScriptedNewsFeed, ScriptedNotifier,
        },
    };
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
    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    #[test]
    fn feed_preserves_source_policies_cursor_absence_and_failure() {
        let metadata = NewsMetadata::new(
            Retention::NotStored,
            AiReadPolicy::TitleOnly,
            ConditionalGet::Supported,
            UserAgentRequirement::Contact,
        );
        let source = FeedSource::News(NewsSource::BankOfKorea);
        let feed = ScriptedNewsFeed::new(source.clone(), metadata.clone());
        contracts::news_feed_source(&feed, &source);
        contracts::news_feed_metadata(&feed, &metadata);
        feed.script_new_items()
            .push(Ok(DataAvailability::Available((vec![], FeedCursor::Start))));
        feed.script_new_items()
            .push(Err(InformationError::Unavailable(ResetTime::Known(at(97)))));
        run(contracts::news_feed_new_items(
            &feed,
            FeedCursor::Start,
            Ok(DataAvailability::Available((vec![], FeedCursor::Start))),
        ));
        run(contracts::news_feed_new_items(
            &feed,
            FeedCursor::Start,
            Err(InformationError::Unavailable(ResetTime::Known(at(97)))),
        ));
        assert_eq!(
            feed.calls(),
            [
                NewsFeedCall::Source(),
                NewsFeedCall::Metadata(),
                NewsFeedCall::NewItems(FeedCursor::Start),
                NewsFeedCall::NewItems(FeedCursor::Start)
            ],
            "메타데이터와 수집 호출 순서"
        );
    }
    #[test]
    fn financial_and_macro_absence_remain_distinct() {
        let fundamentals = ScriptedFundamentals::default();
        let macro_series = ScriptedMacroSeries::default();
        let symbol = Symbol::try_new("FIXTURE").expect("종목");
        let day = at(86_411).offset_calendar_date();
        let series = MacroSeriesId::try_new("fixture-series").expect("계열");
        fundamentals
            .script_statements()
            .push(Ok(DataAvailability::NotConfigured));
        macro_series
            .script_daily_values()
            .push(Ok(DataAvailability::NoData));
        run(contracts::fundamentals_statements(
            &fundamentals,
            &symbol,
            day,
            Ok(DataAvailability::NotConfigured),
        ));
        run(contracts::macro_series_daily_values(
            &macro_series,
            &series,
            day,
            day,
            Ok(DataAvailability::NoData),
        ));
        assert_eq!(
            fundamentals.calls(),
            [FundamentalsCall::Statements(symbol, day)],
            "재무 요청 보존"
        );
        assert_eq!(
            macro_series.calls(),
            [MacroSeriesCall::DailyValues(series, day, day)],
            "거시 요청 보존"
        );
    }
    #[test]
    fn notifier_records_order_and_capabilities_are_explicit_values() {
        let notifier = ScriptedNotifier::default();
        notifier
            .script_notify()
            .push(Ok(application::WriteCompleted));
        run(contracts::notifier_notify(
            &notifier,
            Notification::UrgentDisclosure,
            Ok(application::WriteCompleted),
        ));
        assert_eq!(
            notifier.calls(),
            [NotifierCall::Notify(Notification::UrgentDisclosure)],
            "알림 기록"
        );
        let capabilities = BrokerCapabilities::new(BTreeSet::from([
            BrokerCapability::Calendar,
            BrokerCapability::ClosedOrders,
        ]));
        contracts::broker_capabilities(
            &capabilities,
            BrokerCapability::Calendar,
            CapabilitySupport::Supported,
        );
        contracts::broker_capabilities(
            &capabilities,
            BrokerCapability::ClosedOrders,
            CapabilitySupport::Supported,
        );
        contracts::broker_capabilities(
            &capabilities,
            BrokerCapability::OpenOrders,
            CapabilitySupport::Unsupported,
        );
    }
}
