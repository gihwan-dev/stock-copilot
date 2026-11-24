#[cfg(test)]
mod tests {
    use application::{
        AiRunId, ComparePromptReplay, EstimateToolArguments, PromptFile, PromptReplayRunner,
        ReplayCase, ReplayCount, ReplayOutput, SystemPortError, ToolCallOutcome, ToolCallRecord,
        ToolCallRecordInput, ToolName, ToolPayloadJson, ToolProtocolVersion, ToolRequestId,
    };
    use domain::{
        AiProbabilityInput, CalibrationEstimateId, EvidenceBundleId, Nanosecond, OffsetSeconds,
        Timestamp, ToolReference, UnixSeconds, UntrustedText,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
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
    fn at() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(100),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn prompt(version: &str) -> PromptFile {
        PromptFile::parse(&format!(
            "---\nid: estimate\nversion: {version}\ntask: estimate\ninputs: task_id\noutput_schema: submit_estimate\ntools: submit_estimate\n---\n본문\n"
        ))
        .expect("프롬프트")
    }
    fn call(request: &str, result: &str) -> ToolCallRecord {
        ToolCallRecord::new(ToolCallRecordInput {
            run_id: AiRunId::try_new("frozen").expect("실행"),
            request_id: ToolRequestId::try_new(request).expect("요청"),
            tool_name: ToolName::try_new("get_quote").expect("도구"),
            arguments_json: ToolPayloadJson::try_new("{}").expect("인자"),
            result_json: ToolPayloadJson::try_new(result).expect("결과"),
            outcome: ToolCallOutcome::Succeeded,
            started_at: at(),
            finished_at: at(),
            protocol_version: ToolProtocolVersion::try_new("2026-07-28").expect("버전"),
        })
    }
    fn case(bundle: &str) -> ReplayCase {
        ReplayCase {
            bundle: EvidenceBundleId::try_new(bundle).expect("묶음"),
            ledger: vec![call("q1", "{\"price\":\"71200\",\"per\":\"12.5\"}")],
        }
    }
    fn submitted(rationale: &str, sources: &[&str]) -> ReplayOutput {
        ReplayOutput::Submitted(EstimateToolArguments {
            task_id: CalibrationEstimateId::try_new("task").expect("과제"),
            probability: AiProbabilityInput::try_new("40").expect("확률"),
            rationale: UntrustedText::try_new(rationale).expect("근거"),
            counterargument: UntrustedText::try_new("반대 논거").expect("반론"),
            sources: sources
                .iter()
                .map(|source| ToolReference::try_new(source).expect("참조"))
                .collect(),
        })
    }

    #[derive(Debug)]
    struct Runner;
    impl PromptReplayRunner for Runner {
        fn replay(
            &self,
            prompt: &PromptFile,
            case: &ReplayCase,
        ) -> impl Future<Output = Result<ReplayOutput, SystemPortError>> + Send {
            let output = match (prompt.version(), case.bundle.as_str()) {
                ("v1", "bundle-a") => submitted("현재가 71200원, PER 12.5배", &["tool:q1"]),
                ("v1", "bundle-b") => submitted("현재가 80000원", &["tool:q1"]),
                ("v1", _) => submitted("근거만 있다", &[]),
                ("v2", "bundle-c") => ReplayOutput::SchemaRejected,
                (_, _) => submitted("현재가 71200원", &["tool:q1"]),
            };
            std::future::ready(Ok(output))
        }
    }

    #[test]
    fn baseline_and_candidate_are_measured_on_the_same_frozen_cases() {
        let cases = [case("bundle-a"), case("bundle-b"), case("bundle-c")];
        let comparison =
            run(ComparePromptReplay::new(&Runner).execute(&prompt("v1"), &prompt("v2"), &cases))
                .expect("비교");
        assert_eq!(comparison.baseline.cases.as_usize(), 3);
        assert_eq!(comparison.baseline.schema_passed.as_usize(), 3);
        assert_eq!(comparison.baseline.with_sources.as_usize(), 2);
        assert_eq!(comparison.baseline.numeric_mismatches.as_usize(), 1);
        assert_eq!(comparison.candidate.cases.as_usize(), 3);
        assert_eq!(comparison.candidate.schema_passed.as_usize(), 2);
        assert_eq!(comparison.candidate.with_sources.as_usize(), 2);
        assert_eq!(
            comparison.candidate.numeric_mismatches,
            ReplayCount::default()
        );
    }

    #[test]
    fn numbers_without_cited_sources_count_as_mismatches() {
        #[derive(Debug)]
        struct Uncited;
        impl PromptReplayRunner for Uncited {
            fn replay(
                &self,
                _prompt: &PromptFile,
                _case: &ReplayCase,
            ) -> impl Future<Output = Result<ReplayOutput, SystemPortError>> + Send {
                std::future::ready(Ok(submitted("현재가 71200원", &[])))
            }
        }
        let cases = [case("bundle-z")];
        let comparison =
            run(ComparePromptReplay::new(&Uncited).execute(&prompt("v1"), &prompt("v2"), &cases))
                .expect("비교");
        assert_eq!(comparison.baseline.numeric_mismatches.as_usize(), 1);
        assert_eq!(comparison.baseline.with_sources.as_usize(), 0);
    }
}
