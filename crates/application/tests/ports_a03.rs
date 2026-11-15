#[cfg(test)]
mod tests {
    use application::{
        AiRunId, BugDiagnostic, ClassifiedError, CollectedObservation, ErrorClass, FinancialLine,
        FinancialStatement, InformationError, MacroObservation, MacroSeriesId, MetricFact,
        MetricSource, MetricUnit, MetricValue, PublicationTime, RequestRejection, ResponseMismatch,
        RetryDelay, RunInput, RunKind, RunSpec,
    };
    use domain::{
        Amount, AmountInput, CalendarDate, CalendarYear, DayNumber, MonthNumber, Nanosecond,
        NormalizedUrl, OffsetSeconds, Symbol, Timestamp, UnixSeconds, UntrustedText,
    };

    fn instant() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(100),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn day() -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(11),
            DayNumber::new(15),
        )
        .expect("기준일")
    }
    fn text() -> UntrustedText {
        UntrustedText::try_new("비밀 원문").expect("본문")
    }
    fn value() -> MetricValue {
        MetricValue::new(Amount::parse(&AmountInput::new("-12.50")).expect("수치"))
    }
    #[test]
    fn news_preserves_domain_item_and_absent_publication_time() {
        let item = domain::CollectedItem::from_web(
            NormalizedUrl::try_new("https://example.com/a").expect("주소"),
            text(),
            Some(text()),
        );
        for published in [
            PublicationTime::NotProvided,
            PublicationTime::Known(instant()),
        ] {
            let result = CollectedObservation::new(item.clone(), published, instant());
            assert_eq!(result.item(), &item);
            assert_eq!(result.published_at(), published);
            assert_eq!(result.collected_at(), instant());
            assert!(!format!("{result:?}").contains("비밀 원문"));
        }
    }
    #[test]
    fn facts_preserve_unit_date_source_and_exact_value() {
        for unit in [
            MetricUnit::Currency(domain::Currency::Krw),
            MetricUnit::Currency(domain::Currency::Usd),
            MetricUnit::Percent,
            MetricUnit::IndexPoints,
            MetricUnit::Multiple,
            MetricUnit::Count,
        ] {
            for source in [
                MetricSource::Dart,
                MetricSource::UsTreasury,
                MetricSource::Fred,
                MetricSource::TossEtf(Symbol::try_new("SPY").expect("ETF")),
            ] {
                let fact = MetricFact::new(value(), unit, day(), source.clone());
                assert_eq!(fact.value(), &value());
                assert_eq!(fact.unit(), unit);
                assert_eq!(fact.as_of(), day());
                assert_eq!(fact.source(), &source);
            }
        }
    }
    fn expected_kind(spec: &RunSpec) -> RunKind {
        match spec.input() {
            RunInput::Collection(_) => RunKind::Collection,
            RunInput::Analysis {
                prompt: _prompt,
                assignments: _assignments,
            } => RunKind::Analysis,
        }
    }
    #[test]
    fn run_kinds_are_collection_and_analysis_without_environment_details() {
        for input in [
            RunInput::Collection(text()),
            RunInput::Analysis {
                prompt: text(),
                assignments: vec![],
            },
        ] {
            let spec = RunSpec::new(AiRunId::try_new("run-1").expect("실행"), input.clone());
            assert_eq!(spec.id().as_str(), "run-1");
            assert_eq!(spec.input(), &input);
            assert_eq!(spec.kind(), expected_kind(&spec));
            assert!(!format!("{spec:?}").contains("비밀 원문"));
        }
    }
    #[test]
    fn identifiers_reject_empty_long_and_non_identifier_text() {
        for invalid in ["", "a/b", "a b", "한글", "a\n", &"a".repeat(129)] {
            assert_eq!(
                AiRunId::try_new(invalid),
                Err(InformationError::InvalidIdentifier)
            );
        }
        for valid in ["a", "run-1", "RUN_2.x", &"a".repeat(128)] {
            assert_eq!(AiRunId::try_new(valid).expect("식별자").as_str(), valid);
        }
    }
    fn expected_class(error: InformationError) -> ErrorClass {
        match error {
            InformationError::Unavailable(retry) => ErrorClass::Wait(retry),
            InformationError::InvalidResponse(reason) => {
                ErrorClass::Bug(BugDiagnostic::Response(reason))
            }
            InformationError::InvalidIdentifier => {
                ErrorClass::Bug(BugDiagnostic::Request(RequestRejection::MalformedRequest))
            }
        }
    }
    #[test]
    fn every_error_class_preserves_closed_reason_and_retry() {
        for error in [
            InformationError::Unavailable(RetryDelay::Unknown),
            InformationError::Unavailable(RetryDelay::Known(
                domain::TimeSpan::try_new(domain::SpanSeconds::new(7), Nanosecond::new(123))
                    .expect("대기"),
            )),
            InformationError::InvalidResponse(ResponseMismatch::Schema),
            InformationError::InvalidResponse(ResponseMismatch::Validation),
            InformationError::InvalidIdentifier,
        ] {
            assert_eq!(error.class(), expected_class(error));
        }
    }
    #[test]
    fn financial_and_macro_envelopes_keep_their_identity_and_fields() {
        let fact = MetricFact::new(value(), MetricUnit::Percent, day(), MetricSource::Fred);
        assert_eq!(
            value().amount(),
            &Amount::parse(&AmountInput::new("-12.50")).expect("수치")
        );
        let line = FinancialLine::new(text(), fact.clone());
        assert_eq!(line.label(), &text());
        assert_eq!(line.fact(), &fact);
        let statement = FinancialStatement::new(
            Symbol::try_new("AAPL").expect("종목"),
            day(),
            vec![line.clone()],
        );
        assert_eq!(statement.symbol(), &Symbol::try_new("AAPL").expect("종목"));
        assert_eq!(statement.period_end(), day());
        assert_eq!(statement.lines(), &vec![line]);
        let series = MacroSeriesId::try_new("DGS10").expect("계열");
        let observation = MacroObservation::new(series.clone(), fact.clone());
        assert_eq!(observation.series(), &series);
        assert_eq!(observation.fact(), &fact);
    }
    fn unit_name(unit: MetricUnit) -> &'static str {
        match unit {
            MetricUnit::Currency(domain::Currency::Krw) => "KRW",
            MetricUnit::Currency(domain::Currency::Usd) => "USD",
            MetricUnit::Percent => "percent",
            MetricUnit::IndexPoints => "points",
            MetricUnit::Multiple => "multiple",
            MetricUnit::Count => "count",
        }
    }
    fn source_name(source: MetricSource) -> &'static str {
        match source {
            MetricSource::Dart => "DART",
            MetricSource::UsTreasury => "Treasury",
            MetricSource::Fred => "FRED",
            MetricSource::TossEtf(_symbol) => "ETF approximation",
        }
    }
    #[test]
    fn units_and_sources_have_exhaustive_contract_expectations() {
        for (unit, expected) in [
            (MetricUnit::Currency(domain::Currency::Krw), "KRW"),
            (MetricUnit::Currency(domain::Currency::Usd), "USD"),
            (MetricUnit::Percent, "percent"),
            (MetricUnit::IndexPoints, "points"),
            (MetricUnit::Multiple, "multiple"),
            (MetricUnit::Count, "count"),
        ] {
            assert_eq!(unit_name(unit), expected);
        }
        for (source, expected) in [
            (MetricSource::Dart, "DART"),
            (MetricSource::UsTreasury, "Treasury"),
            (MetricSource::Fred, "FRED"),
            (
                MetricSource::TossEtf(Symbol::try_new("SPY").expect("ETF")),
                "ETF approximation",
            ),
        ] {
            assert_eq!(source_name(source), expected);
        }
    }
}
