#[cfg(test)]
mod tests {
    use application::{
        AiEvent, AiRunId, AiUsage, AnalysisPrompt, BugDiagnostic, ChatHistory, ChatMessage,
        ChatRole, ClassifiedError, CollectedEntry, CollectedObservation, CollectionQuery,
        CollectionTopic, CredentialSource, ErrorClass, FinancialFact, FinancialLine,
        FinancialSource, FinancialStatement, FinancialUnit, IdentifierError, InformationError,
        MacroFact, MacroObservation, MacroSeriesId, MacroSource, MacroUnit, MetricValue, ModelName,
        ModelSelection, PromptError, PublicationTime, ReasoningEffort, ResetTime, ResponseMismatch,
        RetryDelay, RunInput, RunKind, RunPurpose, RunSpec, StoredUserMessage, TokenCount,
        TokenUsage, UserAction, UserMessageId, Utilization,
    };
    use domain::{
        Amount, AmountInput, CalendarDate, CalendarYear, DayNumber, MonthNumber, Nanosecond,
        NormalizedUrl, OffsetSeconds, Symbol, Timestamp, UnixSeconds, UntrustedText,
    };
    fn stamp(value: UnixSeconds) -> Timestamp {
        Timestamp::from_parts(value, Nanosecond::new(0), OffsetSeconds::new(0)).expect("시각")
    }
    fn day(value: DayNumber) -> CalendarDate {
        CalendarDate::from_parts(CalendarYear::new(2026), MonthNumber::new(11), value)
            .expect("날짜")
    }
    fn text(value: &str) -> UntrustedText {
        UntrustedText::try_new(value).expect("텍스트")
    }
    fn value() -> MetricValue {
        MetricValue::new(Amount::parse(&AmountInput::new("-12.50")).expect("수치"))
    }
    #[test]
    fn identifiers_check_every_ascii_character_and_length_boundaries() {
        let allowed = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.-";
        for byte in 0_u8..=127 {
            let character = char::from(byte);
            let input = format!("x{character}y");
            if allowed.contains(character) {
                assert_eq!(AiRunId::try_new(&input).expect("실행 ID").as_str(), input);
                assert_eq!(
                    MacroSeriesId::try_new(&input).expect("계열 ID").as_str(),
                    input
                );
                assert_eq!(ModelName::try_new(&input).expect("모델").as_str(), input);
                assert_eq!(
                    UserMessageId::try_new(&input).expect("메시지 ID").as_str(),
                    input
                );
            } else {
                assert_bad_identifiers(&input);
            }
        }
        for invalid in ["", "한글", "é", "a+b", "a@b", "a:b", &"z".repeat(129)] {
            assert_bad_identifiers(invalid);
        }
        for valid in ["0", "a", "_", ".", "-", &"z".repeat(128)] {
            assert_eq!(AiRunId::try_new(valid).expect("실행 ID").as_str(), valid);
            assert_eq!(
                MacroSeriesId::try_new(valid).expect("계열 ID").as_str(),
                valid
            );
            assert_eq!(ModelName::try_new(valid).expect("모델").as_str(), valid);
            assert_eq!(
                UserMessageId::try_new(valid).expect("메시지 ID").as_str(),
                valid
            );
        }
    }
    fn assert_bad_identifiers(input: &str) {
        assert_eq!(AiRunId::try_new(input), Err(IdentifierError::InvalidFormat));
        assert_eq!(
            MacroSeriesId::try_new(input),
            Err(IdentifierError::InvalidFormat)
        );
        assert_eq!(
            ModelName::try_new(input),
            Err(IdentifierError::InvalidFormat)
        );
        assert_eq!(
            UserMessageId::try_new(input),
            Err(IdentifierError::InvalidFormat)
        );
    }
    #[test]
    fn news_and_disclosures_preserve_identity_and_distinct_times() {
        let receipts = [
            domain::ReceiptNo::Dart(
                domain::DartReceiptNo::try_new("20261115000001").expect("DART"),
            ),
            domain::ReceiptNo::Kind(
                domain::KindReceiptNo::try_new("20261115000001").expect("KIND"),
            ),
            domain::ReceiptNo::SecEdgar(
                domain::SecEdgarReceiptNo::try_new("0000320193-26-000003").expect("SEC"),
            ),
        ];
        let mut keys = vec![];
        for receipt in receipts {
            let disclosure = domain::Disclosure::new(
                receipt.clone(),
                NormalizedUrl::try_new("https://example.com/disclosure").expect("주소"),
                text("공시 제목"),
                Some(text("비밀 본문")),
            );
            let observed = CollectedObservation::new(
                CollectedEntry::Disclosure(disclosure),
                PublicationTime::Known(stamp(UnixSeconds::new(11))),
                stamp(UnixSeconds::new(99)),
            );
            let CollectedEntry::Disclosure(item) = observed.entry() else {
                panic!("공시 키를 보존해야 함")
            };
            assert_eq!(item.key().receipt_no(), &receipt);
            assert_eq!(item.key().source(), receipt.source());
            keys.push(item.key().clone());
            assert_times_and_redaction(&observed);
        }
        assert_ne!(keys.first(), keys.get(1));
    }
    #[test]
    fn news_sources_preserve_identity_and_distinct_times() {
        for source in [
            domain::NewsSource::Hankyung,
            domain::NewsSource::Maekyung,
            domain::NewsSource::Yonhap,
            domain::NewsSource::Edaily,
            domain::NewsSource::BankOfKorea,
            domain::NewsSource::FederalReserve,
            domain::NewsSource::GoogleNews,
            domain::NewsSource::YahooFinance,
        ] {
            let news = domain::NewsItem::new(
                source.clone(),
                NormalizedUrl::try_new("https://example.com/news").expect("주소"),
                text("기사"),
                Some(text("비밀 본문")),
            );
            let observed = CollectedObservation::new(
                CollectedEntry::News(news),
                PublicationTime::Known(stamp(UnixSeconds::new(11))),
                stamp(UnixSeconds::new(99)),
            );
            let CollectedEntry::News(item) = observed.entry() else {
                panic!("기사를 보존해야 함")
            };
            assert_eq!(
                item.collected().source(),
                &domain::InformationSource::News(source)
            );
            assert_eq!(item.collected().url().as_str(), "https://example.com/news");
            assert_times_and_redaction(&observed);
        }
    }
    fn assert_times_and_redaction(observed: &CollectedObservation) {
        assert_eq!(
            observed.published_at(),
            PublicationTime::Known(stamp(UnixSeconds::new(11)))
        );
        assert_eq!(observed.collected_at(), stamp(UnixSeconds::new(99)));
        assert!(!format!("{observed:?}").contains("비밀 본문"));
    }
    #[test]
    fn missing_publication_does_not_replace_collection_time() {
        let news = domain::NewsItem::new(
            domain::NewsSource::Edaily,
            NormalizedUrl::try_new("https://example.com/none").expect("주소"),
            text("기사"),
            None,
        );
        let observed = CollectedObservation::new(
            CollectedEntry::News(news),
            PublicationTime::NotProvided,
            stamp(UnixSeconds::new(42)),
        );
        assert_eq!(observed.published_at(), PublicationTime::NotProvided);
        assert_eq!(observed.collected_at(), stamp(UnixSeconds::new(42)));
    }
    #[test]
    fn financial_period_symbol_and_row_date_remain_independent() {
        for name in ["MSFT", "NVDA"] {
            for unit in [
                FinancialUnit::Currency(domain::Currency::Krw),
                FinancialUnit::Currency(domain::Currency::Usd),
                FinancialUnit::Percent,
                FinancialUnit::Multiple,
                FinancialUnit::Count,
            ] {
                let fact = FinancialFact::new(
                    value(),
                    unit,
                    day(DayNumber::new(10)),
                    FinancialSource::Dart,
                );
                assert_eq!(
                    fact.value().amount(),
                    &Amount::parse(&AmountInput::new("-12.50")).expect("수치")
                );
                assert_eq!(fact.unit(), unit);
                assert_eq!(fact.source(), FinancialSource::Dart);
                assert_eq!(fact.as_of(), day(DayNumber::new(10)));
                let line = FinancialLine::new(text("외부 계정명"), fact.clone());
                assert_eq!(line.label(), &text("외부 계정명"));
                assert_eq!(line.fact(), &fact);
                let statement = FinancialStatement::new(
                    Symbol::try_new(name).expect("종목"),
                    day(DayNumber::new(15)),
                    vec![line.clone()],
                );
                assert_eq!(statement.symbol().as_str(), name);
                assert_eq!(statement.period_end(), day(DayNumber::new(15)));
                assert_eq!(statement.lines(), &vec![line]);
            }
        }
    }
    #[test]
    fn macro_facts_preserve_etf_symbol_and_official_provenance() {
        for source in [
            MacroSource::UsTreasury,
            MacroSource::Fred,
            MacroSource::TossEtf(Symbol::try_new("QQQ").expect("ETF")),
            MacroSource::TossEtf(Symbol::try_new("VT").expect("ETF")),
        ] {
            for unit in [MacroUnit::Percent, MacroUnit::IndexPoints, MacroUnit::Usd] {
                let fact = MacroFact::new(value(), unit, day(DayNumber::new(12)), source.clone());
                assert_eq!(fact.value(), &value());
                assert_eq!(fact.unit(), unit);
                assert_eq!(fact.source(), &source);
                assert_eq!(fact.as_of(), day(DayNumber::new(12)));
                let observation = MacroObservation::new(
                    MacroSeriesId::try_new("DGS2").expect("계열"),
                    fact.clone(),
                );
                assert_eq!(observation.series().as_str(), "DGS2");
                assert_eq!(observation.fact(), &fact);
            }
        }
    }
    #[test]
    fn errors_preserve_credential_source_and_absolute_reset_time() {
        for source in [CredentialSource::Dart, CredentialSource::Fred] {
            let error = InformationError::InvalidCredential(source);
            assert_eq!(
                error.class(),
                ErrorClass::UserAction(UserAction::CheckCredentials)
            );
            let InformationError::InvalidCredential(actual) = error else {
                panic!("자격 원인 보존")
            };
            assert_eq!(actual, source);
        }
        for resets in [
            ResetTime::NotProvided,
            ResetTime::Known(stamp(UnixSeconds::new(321))),
        ] {
            let error = InformationError::Unavailable(resets);
            assert_eq!(error.class(), ErrorClass::Wait(RetryDelay::Unknown));
            let InformationError::Unavailable(actual) = error else {
                panic!("절대 시각 보존")
            };
            assert_eq!(actual, resets);
        }
        for mismatch in [ResponseMismatch::Schema, ResponseMismatch::Validation] {
            assert_eq!(
                InformationError::InvalidResponse(mismatch).class(),
                ErrorClass::Bug(BugDiagnostic::Response(mismatch))
            );
        }
    }
    #[test]
    fn collection_queries_separate_code_topics_from_stored_user_messages() {
        let query = CollectionQuery::CodeTopics {
            instruments: vec![Symbol::try_new("MSFT").expect("종목")],
            topics: vec![
                CollectionTopic::CompanyNews,
                CollectionTopic::Earnings,
                CollectionTopic::Disclosures,
            ],
        };
        let spec = spec(RunInput::Collection(query.clone()));
        assert_eq!(spec.kind(), RunKind::Collection);
        assert_eq!(spec.input(), &RunInput::Collection(query));
        let stored = StoredUserMessage::from_store_result(
            UserMessageId::try_new("message-7").expect("ID"),
            text("사용자가 직접 저장한 원문 123"),
        );
        assert_eq!(stored.id().as_str(), "message-7");
        assert_eq!(
            stored.text().as_untrusted_str(),
            "사용자가 직접 저장한 원문 123"
        );
        let input = RunInput::Collection(CollectionQuery::UserMessage(stored));
        assert_eq!(spec_with(input.clone()).input(), &input);
    }
    fn spec(input: RunInput) -> RunSpec {
        spec_with(input)
    }
    fn spec_with(input: RunInput) -> RunSpec {
        RunSpec::new(
            AiRunId::try_new("run-42").expect("실행"),
            input,
            ModelSelection::Named(ModelName::try_new("claude-opus-5-5").expect("모델")),
            ReasoningEffort::High,
            RunPurpose::DeepResearch,
        )
    }
    #[test]
    fn analysis_spec_and_prompt_preserve_model_effort_and_purpose() {
        let prompt = AnalysisPrompt::try_new("비밀 분석 프롬프트").expect("프롬프트");
        assert_eq!(prompt.as_str(), "비밀 분석 프롬프트");
        let spec = spec(RunInput::Analysis(prompt));
        assert_eq!(spec.kind(), RunKind::Analysis);
        assert_eq!(spec.id().as_str(), "run-42");
        assert_eq!(
            spec.model(),
            &ModelSelection::Named(ModelName::try_new("claude-opus-5-5").expect("모델"))
        );
        assert_eq!(spec.effort(), ReasoningEffort::High);
        assert_eq!(spec.purpose(), RunPurpose::DeepResearch);
        assert!(!format!("{spec:?}").contains("비밀 분석 프롬프트"));
        assert_eq!(AnalysisPrompt::try_new(""), Err(PromptError::Empty));
        assert_eq!(
            AnalysisPrompt::try_new(&"x".repeat(65_537)),
            Err(PromptError::TooLong)
        );
        assert_eq!(
            AnalysisPrompt::try_new(&"x".repeat(65_536))
                .expect("상한")
                .as_str()
                .len(),
            65_536
        );
    }
    #[test]
    fn history_keeps_roles_order_and_exact_untrusted_text() {
        let history = ChatHistory::new(vec![
            ChatMessage::new(ChatRole::User, text("사용자\u{200b}")),
            ChatMessage::new(ChatRole::Assistant, text("비서")),
            ChatMessage::new(ChatRole::User, text("다음 사용자")),
        ]);
        assert_eq!(
            history
                .messages()
                .iter()
                .map(ChatMessage::role)
                .collect::<Vec<_>>(),
            vec![ChatRole::User, ChatRole::Assistant, ChatRole::User]
        );
        assert_eq!(
            history
                .messages()
                .iter()
                .map(|message| message.text().as_untrusted_str())
                .collect::<Vec<_>>(),
            vec!["사용자\u{200b}", "비서", "다음 사용자"]
        );
        assert!(!format!("{history:?}").contains("다음 사용자"));
        assert_eq!(ChatHistory::new(vec![]).messages(), &[]);
    }
    #[test]
    fn usage_reuses_domain_zero_to_one_validation_and_absolute_reset() {
        for number in ["0", "0.001", "0.7", "1"] {
            let ratio = domain::Ratio::parse(&domain::NumericInput::try_new(number).expect("숫자"))
                .expect("비율");
            let probability = domain::Probability::try_new(ratio.as_decimal()).expect("0~1");
            let utilization = Utilization::new(probability);
            assert_eq!(utilization.as_probability(), &probability);
            let AiUsage::Available(actual) = AiUsage::Available(utilization) else {
                panic!("사용률 보존")
            };
            assert_eq!(actual.as_probability(), &probability);
        }
        for number in ["-0.001", "1.001"] {
            let ratio = domain::Ratio::parse(&domain::NumericInput::try_new(number).expect("숫자"))
                .expect("비율");
            assert_eq!(
                domain::Probability::try_new(ratio.as_decimal()),
                Err(domain::ProbabilityError::OutOfRange)
            );
        }
        let resets = ResetTime::Known(stamp(UnixSeconds::new(404)));
        assert_eq!(
            AiUsage::Waiting(resets),
            AiUsage::Waiting(ResetTime::Known(stamp(UnixSeconds::new(404))))
        );
    }
    #[test]
    fn tokens_and_actual_model_are_distinct_result_facts() {
        let usage = TokenUsage::new(
            TokenCount::new(11),
            TokenCount::new(22),
            TokenCount::new(33),
            TokenCount::new(44),
        );
        assert_eq!(*usage.input().as_u64(), 11);
        assert_eq!(*usage.output().as_u64(), 22);
        assert_eq!(*usage.cache_read().as_u64(), 33);
        assert_eq!(*usage.cache_creation().as_u64(), 44);
        let model = ModelName::try_new("claude-sonnet-5-5").expect("실제 모델");
        let event = AiEvent::ActualModel(model.clone());
        assert_eq!(event, AiEvent::ActualModel(model));
        assert_ne!(
            event,
            AiEvent::ActualModel(ModelName::try_new("claude-opus-5-5").expect("요청 모델"))
        );
    }
}
