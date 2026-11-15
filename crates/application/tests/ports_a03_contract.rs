#[cfg(test)]
mod tests {
    use application::{
        AiEvent, AiEventStream, AiProvider, AiReadPolicy, AiRunId, AiUsage, ChatHistory,
        ChatMessage, ChatRole, CollectedObservation, CollectionQuery, CollectionTopic,
        ConditionalGet, DataAvailability, FeedCursor, FeedPosition, FeedSource, FinancialStatement,
        Fundamentals, InformationError, IsolationCheck, IsolationFailure, MacroObservation,
        MacroSeries, MacroSeriesId, ModelName, ModelSelection, NewsFeed, NewsMetadata,
        ReasoningEffort, ResetTime, Retention, RunInput, RunKind, RunPurpose, RunSpec, TokenCount,
        TokenUsage, ToolStatus, UserAgentRequirement,
    };
    use domain::{CalendarDate, Symbol, UntrustedText};
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    struct Contract(IsolationCheck);
    struct Stream(std::vec::IntoIter<AiEvent>);
    fn poll_ready<T>(future: impl Future<Output = T> + Send) -> T {
        match std::pin::pin!(future).poll(&mut Context::from_waker(Waker::noop())) {
            Poll::Ready(value) => Some(value),
            Poll::Pending => None,
        }
        .expect("즉시 응답 대본")
    }
    fn output(check: IsolationCheck) -> Stream {
        let events = match check {
            IsolationCheck::Passed => vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                AiEvent::TextDelta(UntrustedText::try_new("분석 응답").expect("응답")),
            ],
            IsolationCheck::Rejected(reason) => {
                vec![AiEvent::IsolationChecked(IsolationCheck::Rejected(reason))]
            }
        };
        Stream(events.into_iter())
    }
    impl NewsFeed for Contract {
        fn source(&self) -> FeedSource {
            FeedSource::Disclosure(domain::DisclosureSource::Kind)
        }
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
            since: FeedCursor,
        ) -> impl Future<
            Output = Result<
                DataAvailability<(Vec<CollectedObservation>, FeedCursor)>,
                InformationError,
            >,
        > + Send {
            std::future::ready(Ok(DataAvailability::Available((vec![], since))))
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
            std::future::ready(Ok(self.0.next()))
        }
    }
    impl AiProvider for Contract {
        type Stream = Stream;
        fn run(
            &self,
            _spec: RunSpec,
        ) -> impl Future<Output = Result<Stream, InformationError>> + Send {
            std::future::ready(Ok(output(self.0)))
        }
        fn resume(
            &self,
            _spec: RunSpec,
            _history: ChatHistory,
            _message: UntrustedText,
        ) -> impl Future<Output = Result<Stream, InformationError>> + Send {
            std::future::ready(Ok(output(self.0)))
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
            std::future::ready(Ok(self.0))
        }
    }
    fn spec() -> RunSpec {
        RunSpec::new(
            AiRunId::try_new("contract-run").expect("실행"),
            RunInput::Collection(CollectionQuery::CodeTopics {
                instruments: vec![Symbol::try_new("MSFT").expect("종목")],
                topics: vec![CollectionTopic::CompanyNews],
            }),
            ModelSelection::Default,
            ReasoningEffort::Default,
            RunPurpose::WebCollection,
        )
    }
    fn day() -> CalendarDate {
        domain::Timestamp::from_parts(
            domain::UnixSeconds::new(0),
            domain::Nanosecond::new(0),
            domain::OffsetSeconds::new(0),
        )
        .expect("시각")
        .offset_calendar_date()
    }
    #[test]
    fn all_port_methods_have_the_contract_types_and_send_futures() {
        let port = Contract(IsolationCheck::Passed);
        assert_eq!(
            port.source(),
            FeedSource::Disclosure(domain::DisclosureSource::Kind)
        );
        assert_eq!(
            poll_ready(port.new_items(FeedCursor::Start)),
            Ok(DataAvailability::Available((vec![], FeedCursor::Start)))
        );
        assert_eq!(
            poll_ready(port.statements(&Symbol::try_new("MSFT").expect("종목"), day())),
            Ok(DataAvailability::NotConfigured)
        );
        assert_eq!(
            poll_ready(port.daily_values(
                &MacroSeriesId::try_new("DGS2").expect("계열"),
                day(),
                day()
            )),
            Ok(DataAvailability::NoData)
        );
        let mut stream = poll_ready(port.run(spec())).expect("실행");
        assert_eq!(
            poll_ready(stream.next()),
            Ok(Some(AiEvent::IsolationChecked(IsolationCheck::Passed)))
        );
        assert_eq!(poll_ready(port.usage()), Ok(AiUsage::Unknown));
        assert_eq!(poll_ready(port.interrupt(spec().id())), Ok(()));
        assert_eq!(
            poll_ready(port.verify_isolation(RunKind::Analysis)),
            Ok(IsolationCheck::Passed)
        );
        assert_eq!(port.metadata().retention(), Retention::NotStored);
        let mut resumed = poll_ready(port.resume(
            spec(),
            ChatHistory::new(vec![ChatMessage::new(
                ChatRole::User,
                UntrustedText::try_new("저장된 대화").expect("대화"),
            )]),
            UntrustedText::try_new("최신 메시지").expect("메시지"),
        ))
        .expect("재개");
        assert_eq!(
            poll_ready(resumed.next()),
            Ok(Some(AiEvent::IsolationChecked(IsolationCheck::Passed)))
        );
    }
    #[test]
    fn rejection_is_first_and_terminal_for_run_and_resume_contract_examples() {
        for failure in [
            IsolationFailure::Tools,
            IsolationFailure::McpServers,
            IsolationFailure::Plugins,
            IsolationFailure::Skills,
            IsolationFailure::SlashCommands,
            IsolationFailure::PermissionMode,
            IsolationFailure::ApiKeySource,
            IsolationFailure::CliVersion,
            IsolationFailure::LoginRequired,
            IsolationFailure::NotInstalled,
        ] {
            let port = Contract(IsolationCheck::Rejected(failure));
            assert_eq!(
                poll_ready(port.verify_isolation(RunKind::Collection)),
                Ok(IsolationCheck::Rejected(failure))
            );
            let mut run = poll_ready(port.run(spec())).expect("점검 스트림");
            assert_eq!(
                poll_ready(run.next()),
                Ok(Some(AiEvent::IsolationChecked(IsolationCheck::Rejected(
                    failure
                ))))
            );
            assert_eq!(poll_ready(run.next()), Ok(None));
            let mut resumed = poll_ready(port.resume(
                spec(),
                ChatHistory::new(vec![]),
                UntrustedText::try_new("메시지").expect("메시지"),
            ))
            .expect("점검 스트림");
            assert_eq!(
                poll_ready(resumed.next()),
                Ok(Some(AiEvent::IsolationChecked(IsolationCheck::Rejected(
                    failure
                ))))
            );
            assert_eq!(poll_ready(resumed.next()), Ok(None));
        }
    }
    #[test]
    fn cursor_payload_is_opaque_preserved_and_redacted() {
        let payload = UntrustedText::try_new("비밀 ETag 및 마지막 접수번호").expect("커서");
        let position = FeedPosition::new(payload.clone());
        assert_eq!(position.as_untrusted_text(), &payload);
        let cursor = FeedCursor::Continue(position);
        let result =
            poll_ready(Contract(IsolationCheck::Passed).new_items(cursor.clone())).expect("피드");
        assert_eq!(
            result,
            DataAvailability::Available((vec![], cursor.clone()))
        );
        assert!(!format!("{cursor:?}").contains("비밀 ETag"));
    }
    #[test]
    fn source_policies_preserve_all_independent_fields() {
        let duration = domain::TimeSpan::try_new(
            domain::SpanSeconds::new(604_800),
            domain::Nanosecond::new(0),
        )
        .expect("기간");
        for retention in [Retention::NotStored, Retention::For(duration)] {
            for ai in [
                AiReadPolicy::TitleOnly,
                AiReadPolicy::TitleAndSummary,
                AiReadPolicy::BodyAllowed,
            ] {
                policies(retention, ai);
            }
        }
    }
    fn policies(retention: Retention, ai: AiReadPolicy) {
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
    fn event_payloads_are_closed_and_external_text_is_redacted() {
        let text = UntrustedText::try_new("검증용 응답 원문").expect("응답");
        let events = vec![
            AiEvent::IsolationChecked(IsolationCheck::Passed),
            AiEvent::TextDelta(text.clone()),
            AiEvent::ToolCall {
                server: text.clone(),
                tool: text.clone(),
                args: text.clone(),
                result: text.clone(),
                status: ToolStatus::Succeeded,
            },
            AiEvent::WebSearch {
                query: text.clone(),
                action: text.clone(),
            },
            AiEvent::TokenUsage(TokenUsage::new(
                TokenCount::new(1),
                TokenCount::new(2),
                TokenCount::new(3),
                TokenCount::new(4),
            )),
            AiEvent::Quota(AiUsage::Unknown),
            AiEvent::RateLimited {
                resets_at: ResetTime::NotProvided,
            },
            AiEvent::ModelRerouted(ModelName::try_new("rerouted-model").expect("모델")),
            AiEvent::ActualModel(ModelName::try_new("actual-model").expect("모델")),
            AiEvent::McpUnavailable,
            AiEvent::Unknown(text.clone()),
        ];
        for event in events {
            assert!(!format!("{event:?}").contains("검증용 응답 원문"));
            match event {
                AiEvent::IsolationChecked(check) => assert_eq!(check, IsolationCheck::Passed),
                AiEvent::TextDelta(actual) | AiEvent::Unknown(actual) => assert_eq!(actual, text),
                AiEvent::ToolCall {
                    server,
                    tool,
                    args,
                    result,
                    status,
                } => {
                    assert_eq!(server, text);
                    assert_eq!(tool, text);
                    assert_eq!(args, text);
                    assert_eq!(result, text);
                    assert_eq!(status, ToolStatus::Succeeded);
                }
                AiEvent::WebSearch { query, action } => {
                    assert_eq!(query, text);
                    assert_eq!(action, text);
                }
                AiEvent::TokenUsage(usage) => assert_eq!(*usage.cache_creation().as_u64(), 4),
                AiEvent::Quota(usage) => assert_eq!(usage, AiUsage::Unknown),
                AiEvent::RateLimited { resets_at } => assert_eq!(resets_at, ResetTime::NotProvided),
                AiEvent::ModelRerouted(model) => assert_eq!(model.as_str(), "rerouted-model"),
                AiEvent::ActualModel(model) => assert_eq!(model.as_str(), "actual-model"),
                AiEvent::McpUnavailable => {}
            }
        }
    }
}
