#[cfg(test)]
mod tests {
    use application::{
        AiEvent, AiEventStream, AiProvider, AiReadPolicy, AiRunId, AiUsage, CollectedObservation,
        ConditionalGet, DataAvailability, FinancialStatement, Fundamentals, InformationError,
        IsolationCheck, MacroObservation, MacroSeries, MacroSeriesId, NewsMetadata, NewsSource,
        ResetTime, Retention, RunInput, RunKind, RunSpec, TokenCount, ToolStatus,
        UserAgentRequirement,
    };
    use domain::{CalendarDate, Symbol, UntrustedText};
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    struct Contract;
    struct Stream;
    fn poll_ready<T>(future: impl Future<Output = T> + Send) -> T {
        match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => Some(value),
            Poll::Pending => None,
        }
        .expect("계약 대본은 즉시 응답")
    }
    impl NewsSource for Contract {
        fn metadata(&self) -> NewsMetadata {
            NewsMetadata::new(
                Retention::NotStored,
                AiReadPolicy::TitleOnly,
                ConditionalGet::Unsupported,
                UserAgentRequirement::Standard,
            )
        }
        fn new_items(
            &self,
        ) -> impl Future<Output = Result<Vec<CollectedObservation>, InformationError>> + Send
        {
            std::future::ready(Ok(vec![]))
        }
    }
    impl Fundamentals for Contract {
        fn statements(
            &self,
            _symbol: &Symbol,
            _period_end: CalendarDate,
        ) -> impl Future<
            Output = Result<DataAvailability<Vec<FinancialStatement>>, InformationError>,
        > + Send {
            std::future::ready(Ok(DataAvailability::NotConfigured))
        }
    }
    impl MacroSeries for Contract {
        fn daily_values(
            &self,
            _series: &MacroSeriesId,
            _from: CalendarDate,
            _through: CalendarDate,
        ) -> impl Future<Output = Result<DataAvailability<Vec<MacroObservation>>, InformationError>> + Send
        {
            std::future::ready(Ok(DataAvailability::NoData))
        }
    }
    impl AiEventStream for Stream {
        fn next(
            &mut self,
        ) -> impl Future<Output = Result<Option<AiEvent>, InformationError>> + Send {
            std::future::ready(Ok(None))
        }
    }
    impl AiProvider for Contract {
        type Stream = Stream;
        fn run(
            &self,
            _spec: RunSpec,
        ) -> impl Future<Output = Result<Stream, InformationError>> + Send {
            std::future::ready(Ok(Stream))
        }
        fn resume(
            &self,
            _spec: RunSpec,
            _message: UntrustedText,
        ) -> impl Future<Output = Result<Stream, InformationError>> + Send {
            std::future::ready(Ok(Stream))
        }
        fn usage(&self) -> impl Future<Output = Result<AiUsage, InformationError>> + Send {
            std::future::ready(Ok(AiUsage::Unknown))
        }
        fn interrupt(
            &self,
            _run: &AiRunId,
        ) -> impl Future<Output = Result<(), InformationError>> + Send {
            std::future::ready(Ok(()))
        }
        fn verify_isolation(
            &self,
            _kind: RunKind,
        ) -> impl Future<Output = Result<IsolationCheck, InformationError>> + Send {
            std::future::ready(Ok(IsolationCheck::Passed))
        }
    }
    #[test]
    fn contract_table_methods_compile_with_send_futures_and_typed_results() {
        let port = Contract;
        let day = domain::Timestamp::from_parts(
            domain::UnixSeconds::new(0),
            domain::Nanosecond::new(0),
            domain::OffsetSeconds::new(0),
        )
        .expect("시각")
        .offset_calendar_date();
        assert_eq!(poll_ready(port.new_items()).expect("새 항목"), vec![]);
        assert_eq!(
            poll_ready(port.statements(&Symbol::try_new("AAPL").expect("종목"), day)),
            Ok(DataAvailability::NotConfigured)
        );
        assert_eq!(
            poll_ready(port.daily_values(
                &MacroSeriesId::try_new("DGS10").expect("계열"),
                day,
                day
            )),
            Ok(DataAvailability::NoData)
        );
        let spec = RunSpec::new(
            AiRunId::try_new("run-1").expect("실행"),
            RunInput::Collection(UntrustedText::try_new("주제").expect("주제")),
        );
        let mut stream = poll_ready(port.run(spec.clone())).expect("실행");
        assert_eq!(poll_ready(stream.next()), Ok(None));
        let mut resumed = poll_ready(port.resume(
            spec.clone(),
            UntrustedText::try_new("저장된 사용자 메시지").expect("메시지"),
        ))
        .expect("재주입");
        assert_eq!(poll_ready(resumed.next()), Ok(None));
        assert_eq!(poll_ready(port.usage()), Ok(AiUsage::Unknown));
        assert_eq!(poll_ready(port.interrupt(spec.id())), Ok(()));
        assert_eq!(
            poll_ready(port.verify_isolation(RunKind::Collection)),
            Ok(IsolationCheck::Passed)
        );
        assert_eq!(port.metadata().retention(), Retention::NotStored);
    }
    #[test]
    fn source_metadata_preserves_each_policy_without_boolean_flags() {
        let duration = domain::TimeSpan::try_new(
            domain::SpanSeconds::new(604_800),
            domain::Nanosecond::new(0),
        )
        .expect("7일");
        for retention in [Retention::NotStored, Retention::For(duration)] {
            for ai in [
                AiReadPolicy::TitleOnly,
                AiReadPolicy::TitleAndSummary,
                AiReadPolicy::BodyAllowed,
            ] {
                verify_metadata(retention, ai);
            }
        }
    }
    fn verify_metadata(retention: Retention, ai: AiReadPolicy) {
        for conditional in [ConditionalGet::Supported, ConditionalGet::Unsupported] {
            for agent in [
                UserAgentRequirement::Standard,
                UserAgentRequirement::Browser,
                UserAgentRequirement::Contact,
            ] {
                let metadata = NewsMetadata::new(retention, ai, conditional, agent);
                assert_eq!(metadata.retention(), retention);
                assert_eq!(metadata.allow_ai_read(), ai);
                assert_eq!(metadata.supports_conditional_get(), conditional);
                assert_eq!(metadata.requires_user_agent(), agent);
            }
        }
    }
    #[test]
    fn macro_series_identifiers_validate_boundaries_and_preserve_values() {
        for invalid in ["", "a/b", "한글", "a b", &"a".repeat(129)] {
            assert_eq!(
                MacroSeriesId::try_new(invalid),
                Err(InformationError::InvalidIdentifier)
            );
        }
        for valid in ["DGS10", "SP500", "a", "x_y.z-0", &"a".repeat(128)] {
            assert_eq!(MacroSeriesId::try_new(valid).expect("계열").as_str(), valid);
        }
    }
    fn event_kind(event: &AiEvent) -> &'static str {
        match event {
            AiEvent::TextDelta(_) => "text",
            AiEvent::ToolCall {
                server: _server,
                tool: _tool,
                args: _args,
                result: _result,
                status: _status,
            } => "tool",
            AiEvent::WebSearch {
                query: _query,
                action: _action,
            } => "web",
            AiEvent::TokenUsage(_) => "tokens",
            AiEvent::Quota(_) => "quota",
            AiEvent::RateLimited {
                resets_at: _resets_at,
            } => "limited",
            AiEvent::ModelRerouted(_) => "model",
            AiEvent::McpUnavailable => "mcp",
            AiEvent::Unknown(_) => "unknown",
        }
    }
    #[test]
    fn architecture_event_variants_are_closed_and_redact_external_text() {
        let secret = UntrustedText::try_new("응답 비밀").expect("원문");
        let events = vec![
            (AiEvent::TextDelta(secret.clone()), "text"),
            (
                AiEvent::ToolCall {
                    server: secret.clone(),
                    tool: secret.clone(),
                    args: secret.clone(),
                    result: secret.clone(),
                    status: ToolStatus::Running,
                },
                "tool",
            ),
            (
                AiEvent::WebSearch {
                    query: secret.clone(),
                    action: secret.clone(),
                },
                "web",
            ),
            (AiEvent::TokenUsage(TokenCount::new(123)), "tokens"),
            (AiEvent::Quota(AiUsage::Unknown), "quota"),
            (
                AiEvent::RateLimited {
                    resets_at: ResetTime::NotProvided,
                },
                "limited",
            ),
            (AiEvent::ModelRerouted(secret.clone()), "model"),
            (AiEvent::McpUnavailable, "mcp"),
            (AiEvent::Unknown(secret), "unknown"),
        ];
        for (event, expected) in events {
            assert_eq!(event_kind(&event), expected);
            assert!(!format!("{event:?}").contains("응답 비밀"));
        }
        assert_eq!(*TokenCount::new(123).as_u64(), 123);
    }
}
