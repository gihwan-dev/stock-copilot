#[cfg(test)]
mod tests {
    use ai_codex::{
        CodexAppServerDecoder, CodexExecDecoder, CodexIsolationExpectation, FieldName,
        SchemaFailure, StreamDecodeError, StreamOrderError, ValidationFailure,
        decode_app_server_lines, decode_app_server_text, decode_exec_lines, decode_exec_text,
    };
    use application::{
        AiEvent, AiUsage, InformationError, IsolationCheck, IsolationFailure, ModelName, ResetTime,
        ResponseMismatch, RunKind, TokenCount, TokenUsage, ToolStatus, Utilization,
    };
    use domain::{
        Amount, AmountInput, Nanosecond, OffsetSeconds, Probability, Timestamp, UnixSeconds,
        UntrustedText,
    };
    use proptest::test_runner::{Config, TestRunner};
    use std::collections::BTreeSet;
    use std::fs;

    fn fixture_text(name: &str) -> String {
        let exe = testkit::test_executable().expect("test executable");
        let root = exe
            .ancestors()
            .find(|dir| dir.join("fixtures").join("ai").is_dir())
            .expect("workspace root with fixtures/ai");
        fs::read_to_string(root.join("fixtures").join("ai").join(name)).expect("read fixture")
    }

    fn analysis_expectation() -> CodexIsolationExpectation {
        CodexIsolationExpectation::new(
            RunKind::Analysis,
            BTreeSet::from([String::from("get_portfolio")]),
            Some(String::from("app")),
            Some(BTreeSet::from([String::from("0.167.1")])),
        )
    }

    fn collection_expectation() -> CodexIsolationExpectation {
        CodexIsolationExpectation::new(
            RunKind::Collection,
            BTreeSet::new(),
            None,
            Some(BTreeSet::from([String::from("0.167.1")])),
        )
    }

    fn util(raw: &str) -> Utilization {
        let input = AmountInput::new(raw);
        let amount = Amount::parse(&input).expect("amount");
        Utilization::new(Probability::try_new(amount.as_decimal()).expect("probability"))
    }

    fn utc_seconds(secs: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(secs),
            Nanosecond::new(0_u32),
            OffsetSeconds::new(0_i32),
        )
        .expect("timestamp")
    }

    fn text(value: &str) -> UntrustedText {
        UntrustedText::try_new(value).expect("untrusted text")
    }

    fn startup_thread_ok() -> &'static str {
        r#"{"id":1,"result":{"sandbox":{"type":"readOnly"},"instructionSources":[],"multiAgentMode":false,"cliVersion":"0.167.1","model":"gpt-6.1-sol"}}"#
    }

    fn startup_mcp_ok() -> &'static str {
        r#"{"id":2,"result":{"data":[{"name":"app","status":"ready","tools":["get_portfolio"]}]}}"#
    }

    #[test]
    fn recorded_app_server_analysis_fixture_matches_shared_ai_event_model() {
        let raw = fixture_text("codex-app-server-analysis.jsonl");
        let events =
            decode_app_server_text(analysis_expectation(), &raw).expect("decode app-server");
        let expected = vec![
            AiEvent::IsolationChecked(IsolationCheck::Passed),
            AiEvent::ToolCall {
                server: text("app"),
                tool: text("get_portfolio"),
                args: text(r#"{"currency":"KRW"}"#),
                result: text(""),
                status: ToolStatus::Running,
            },
            AiEvent::ToolCall {
                server: text("app"),
                tool: text("get_portfolio"),
                args: text(r#"{"currency":"KRW"}"#),
                result: text(r#"{"holdings":[{"symbol":"TEST","weight":"0.60"}]}"#),
                status: ToolStatus::Succeeded,
            },
            AiEvent::Quota(AiUsage::Available(util("0.11"))),
            AiEvent::TextDelta(text("TEST 60%")),
            AiEvent::TokenUsage(TokenUsage::new(
                TokenCount::new(1200_u64),
                TokenCount::new(340_u64),
                TokenCount::new(500_u64),
                TokenCount::new(80_u64),
            )),
            AiEvent::ActualModel(ModelName::try_new("gpt-6.1-sol").expect("model")),
        ];
        assert_eq!(events, expected);
    }

    #[test]
    fn recorded_app_server_rate_limit_fixture_preserves_waiting_and_reset() {
        let raw = fixture_text("codex-app-server-rate-limited.jsonl");
        let events =
            decode_app_server_text(analysis_expectation(), &raw).expect("decode rate limit");
        let reset = ResetTime::Known(utc_seconds(1_794_394_800_i64));
        assert_eq!(
            events,
            vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                AiEvent::Quota(AiUsage::Waiting(reset)),
                AiEvent::RateLimited { resets_at: reset },
                AiEvent::RateLimited { resets_at: reset },
            ]
        );
    }

    #[test]
    fn recorded_exec_jsonl_fixture_maps_to_shared_ai_event_model() {
        let raw = fixture_text("codex-exec-analysis.jsonl");
        let events = decode_exec_text(IsolationCheck::Passed, &raw).expect("decode exec");
        assert_eq!(
            events,
            vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                AiEvent::Unknown(text(
                    r#"{"type":"thread.started","thread_id":"thread-fixture-001"}"#
                )),
                AiEvent::Unknown(text(r#"{"type":"turn.started"}"#)),
                AiEvent::ToolCall {
                    server: text("app"),
                    tool: text("get_portfolio"),
                    args: text(r#"{"currency":"KRW"}"#),
                    result: text(""),
                    status: ToolStatus::Running,
                },
                AiEvent::ToolCall {
                    server: text("app"),
                    tool: text("get_portfolio"),
                    args: text(r#"{"currency":"KRW"}"#),
                    result: text(r#"{"holdings":[{"symbol":"TEST","weight":"0.60"}]}"#),
                    status: ToolStatus::Succeeded,
                },
                AiEvent::WebSearch {
                    query: text(""),
                    action: text("other"),
                },
                AiEvent::WebSearch {
                    query: text("KOSPI index today"),
                    action: text("search"),
                },
                AiEvent::TextDelta(text("TEST 60%")),
                AiEvent::TokenUsage(TokenUsage::new(
                    TokenCount::new(1200_u64),
                    TokenCount::new(340_u64),
                    TokenCount::new(500_u64),
                    TokenCount::new(80_u64),
                )),
                AiEvent::ActualModel(ModelName::try_new("gpt-6.1-sol").expect("model")),
            ]
        );
    }

    #[test]
    fn all_eleven_ai_event_variants_and_null_ordinary_usage_are_handled() {
        let events = decode_app_server_lines(analysis_expectation(), all_eleven_codex_lines())
            .expect("decode all");
        assert_codex_model_and_tool_events(&events);
        assert_codex_quota_and_control_events(&events);
    }

    fn all_eleven_codex_lines() -> [&'static str; 13] {
        [
            startup_thread_ok(),
            startup_mcp_ok(),
            r#"{"method":"model/rerouted","params":{"toModel":"gpt-6-luna"}}"#,
            r#"{"method":"item/started","params":{"item":{"id":"m1","type":"mcpToolCall","server":"app","tool":"get_portfolio","arguments":{"x":1},"status":"inProgress"}}}"#,
            r#"{"method":"item/completed","params":{"item":{"id":"m1","type":"mcpToolCall","server":"app","tool":"get_portfolio","arguments":{"x":1},"error":"denied","status":"failed"}}}"#,
            r#"{"method":"item/completed","params":{"item":{"id":"w1","type":"webSearch","query":"KOSPI","action":{"type":"search"}}}}"#,
            r#"{"method":"item/agentMessage/delta","params":{"delta":"hello"}}"#,
            r#"{"method":"account/rateLimits/updated","params":{"ordinaryUsageAllowed":null,"rateLimits":{"primary":{"usedPercent":50,"resetsAt":1794906000}}}}"#,
            r#"{"method":"account/rateLimits/updated","params":{"ordinaryUsageAllowed":true,"rateLimits":{"primary":{"usedPercent":7.5,"resetsAt":1794906000}}}}"#,
            r#"{"method":"account/rateLimits/updated","params":{"ordinaryUsageAllowed":false,"rateLimits":{"primary":{"usedPercent":100,"resetsAt":1794906000}}}}"#,
            r#"{"method":"mcpServer/startupStatus/updated","params":{"name":"app","status":"failed"}}"#,
            r#"{"method":"thread/tokenUsage/updated","params":{"last":{"inputTokens":11,"outputTokens":22,"cachedInputTokens":33,"cacheWriteInputTokens":44}}}"#,
            r#"{"method":"future/codexEvent","params":{"v":1}}"#,
        ]
    }

    fn assert_codex_model_and_tool_events(events: &[AiEvent]) {
        assert!(
            events.contains(&AiEvent::IsolationChecked(IsolationCheck::Passed)),
            "IsolationChecked"
        );
        assert!(
            events.contains(&AiEvent::ModelRerouted(
                ModelName::try_new("gpt-6-luna").expect("rerouted")
            )),
            "ModelRerouted"
        );
        assert!(
            events.contains(&AiEvent::ActualModel(
                ModelName::try_new("gpt-6-luna").expect("actual")
            )),
            "ActualModel"
        );
        assert!(
            events.contains(&AiEvent::ToolCall {
                server: text("app"),
                tool: text("get_portfolio"),
                args: text(r#"{"x":1}"#),
                result: text("denied"),
                status: ToolStatus::Failed,
            }),
            "ToolCall failed"
        );
        assert!(
            events.contains(&AiEvent::WebSearch {
                query: text("KOSPI"),
                action: text("search"),
            }),
            "WebSearch"
        );
    }

    fn assert_codex_quota_and_control_events(events: &[AiEvent]) {
        assert!(
            events.contains(&AiEvent::TextDelta(text("hello"))),
            "TextDelta"
        );
        assert!(
            events.contains(&AiEvent::Quota(AiUsage::Unknown)),
            "null ordinaryUsageAllowed MUST produce Quota::Unknown"
        );
        assert!(
            events.contains(&AiEvent::Quota(AiUsage::Available(util("0.075")))),
            "7.5% -> 0.075"
        );
        assert!(
            events.contains(&AiEvent::RateLimited {
                resets_at: ResetTime::Known(utc_seconds(1_794_906_000_i64)),
            }),
            "RateLimited"
        );
        assert!(events.contains(&AiEvent::McpUnavailable), "McpUnavailable");
        assert!(
            events.contains(&AiEvent::TokenUsage(TokenUsage::new(
                TokenCount::new(11_u64),
                TokenCount::new(22_u64),
                TokenCount::new(33_u64),
                TokenCount::new(44_u64),
            ))),
            "TokenUsage"
        );
        assert!(
            events.contains(&AiEvent::Unknown(text(
                r#"{"method":"future/codexEvent","params":{"v":1}}"#
            ))),
            "Unknown"
        );
    }

    fn codex_isolation_rejection_cases() -> [(
        &'static str,
        &'static str,
        CodexIsolationExpectation,
        IsolationFailure,
    ); 6] {
        [
            (
                r#"{"id":1,"result":{"sandbox":{"type":"workspaceWrite"},"instructionSources":[],"multiAgentMode":false,"cliVersion":"0.167.1"}}"#,
                startup_mcp_ok(),
                analysis_expectation(),
                IsolationFailure::PermissionMode,
            ),
            (
                r#"{"id":1,"result":{"sandbox":{"type":"readOnly"},"instructionSources":["AGENTS.md"],"multiAgentMode":false,"cliVersion":"0.167.1"}}"#,
                startup_mcp_ok(),
                analysis_expectation(),
                IsolationFailure::Skills,
            ),
            (
                r#"{"id":1,"result":{"sandbox":{"type":"readOnly"},"instructionSources":[],"multiAgentMode":true,"cliVersion":"0.167.1"}}"#,
                startup_mcp_ok(),
                analysis_expectation(),
                IsolationFailure::Plugins,
            ),
            (
                r#"{"id":1,"result":{"sandbox":{"type":"readOnly"},"instructionSources":[],"multiAgentMode":false,"cliVersion":"0.999.0"}}"#,
                startup_mcp_ok(),
                analysis_expectation(),
                IsolationFailure::CliVersion,
            ),
            (
                startup_thread_ok(),
                r#"{"id":2,"result":{"data":[{"name":"app","status":"failed","tools":["get_portfolio"]}]}}"#,
                analysis_expectation(),
                IsolationFailure::McpServers,
            ),
            (
                startup_thread_ok(),
                r#"{"id":2,"result":{"data":[{"name":"app","status":"ready","tools":["get_portfolio","place_order"]}]}}"#,
                analysis_expectation(),
                IsolationFailure::Tools,
            ),
        ]
    }

    #[test]
    fn codex_isolation_rejections_terminate_stream_and_block_subsequent_lines() {
        for (thread_line, mcp_line, expectation, failure) in codex_isolation_rejection_cases() {
            let mut decoder = CodexAppServerDecoder::new(expectation);
            assert_eq!(decoder.decode_line(thread_line).expect("thread"), vec![]);
            assert_eq!(
                decoder.decode_line(mcp_line).expect("mcp"),
                vec![AiEvent::IsolationChecked(IsolationCheck::Rejected(failure))]
            );
            assert_eq!(
                decoder
                    .decode_line(r#"{"method":"item/agentMessage/delta","params":{"delta":"x"}}"#),
                Err(StreamDecodeError::Order(
                    StreamOrderError::EventAfterRejection
                ))
            );
            decoder.finish().expect("rejected stream finishes cleanly");
        }

        let collection_with_mcp = [startup_thread_ok(), startup_mcp_ok()];
        assert_eq!(
            decode_app_server_lines(collection_expectation(), collection_with_mcp)
                .expect("collection rejects mcp"),
            vec![AiEvent::IsolationChecked(IsolationCheck::Rejected(
                IsolationFailure::McpServers
            ))]
        );

        let unauthorized =
            [r#"{"id":1,"error":{"codexErrorInfo":"unauthorized","message":"login"}}"#];
        assert_eq!(
            decode_app_server_lines(analysis_expectation(), unauthorized).expect("unauth"),
            vec![AiEvent::IsolationChecked(IsolationCheck::Rejected(
                IsolationFailure::LoginRequired
            ))]
        );
    }

    #[test]
    fn malformed_and_envelope_errors_are_preserved_and_redacted() {
        let mut decoder = CodexAppServerDecoder::new(analysis_expectation());
        let secret = "{broken CODEX_SECRET_999";
        let malformed = decoder.decode_line(secret).expect_err("malformed");
        assert_eq!(
            malformed,
            StreamDecodeError::MalformedJson { raw: text(secret) }
        );
        assert_eq!(
            malformed.to_information_error(),
            InformationError::InvalidResponse(ResponseMismatch::Schema)
        );
        assert!(
            !format!("{malformed:?}").contains("CODEX_SECRET_999"),
            "debug redacts raw"
        );
        assert!(
            !format!("{malformed}").contains("CODEX_SECRET_999"),
            "display redacts raw"
        );
        assert_eq!(
            decoder.decode_line(r#"{"neither":1}"#),
            Err(StreamDecodeError::Schema(
                SchemaFailure::UnrecognizedEnvelope
            ))
        );
        assert_eq!(
            decoder
                .decode_line(r#"{"method":"item/agentMessage/delta","params":{"delta":"early"}}"#),
            Err(StreamDecodeError::Order(
                StreamOrderError::MissingInitialIsolationCheck
            ))
        );
        assert_eq!(
            decoder.finish(),
            Err(StreamDecodeError::Order(
                StreamOrderError::MissingInitialIsolationCheck
            ))
        );
    }

    #[test]
    fn codex_validation_and_order_errors_are_rejected() {
        let bad_percent = [
            startup_thread_ok(),
            startup_mcp_ok(),
            r#"{"method":"account/rateLimits/updated","params":{"ordinaryUsageAllowed":true,"rateLimits":{"primary":{"usedPercent":101}}}}"#,
        ];
        assert_eq!(
            decode_app_server_lines(analysis_expectation(), bad_percent),
            Err(StreamDecodeError::Validation(
                ValidationFailure::InvalidUtilization
            ))
        );
        let orphan_completion = [
            startup_thread_ok(),
            startup_mcp_ok(),
            r#"{"method":"item/completed","params":{"item":{"id":"ghost","type":"mcpToolCall","server":"app","tool":"get_portfolio","arguments":{},"status":"completed"}}}"#,
        ];
        assert_eq!(
            decode_app_server_lines(analysis_expectation(), orphan_completion),
            Err(StreamDecodeError::Order(
                StreamOrderError::UnexpectedToolCompletion
            ))
        );
        let missing_params = [
            startup_thread_ok(),
            startup_mcp_ok(),
            r#"{"method":"item/started"}"#,
        ];
        assert_eq!(
            decode_app_server_lines(analysis_expectation(), missing_params),
            Err(StreamDecodeError::Schema(SchemaFailure::MissingField(
                FieldName::Params
            )))
        );
    }

    #[test]
    fn codex_fail_closed_status_network_and_exec_rejection() {
        let missing_mcp_status = [
            startup_thread_ok(),
            r#"{"id":2,"result":{"data":[{"name":"app","tools":["get_portfolio"]}]}}"#,
        ];
        assert_eq!(
            decode_app_server_lines(analysis_expectation(), missing_mcp_status),
            Err(StreamDecodeError::Schema(SchemaFailure::MissingField(
                FieldName::Status
            )))
        );
        let network_open_for_analysis = [
            r#"{"id":1,"result":{"sandbox":{"type":"readOnly","networkAccess":true},"instructionSources":[],"multiAgentMode":false,"cliVersion":"0.167.1"}}"#,
            startup_mcp_ok(),
        ];
        assert_eq!(
            decode_app_server_lines(analysis_expectation(), network_open_for_analysis)
                .expect("network open rejected"),
            vec![AiEvent::IsolationChecked(IsolationCheck::Rejected(
                IsolationFailure::PermissionMode
            ))]
        );
        let mut rejected_exec =
            CodexExecDecoder::new(IsolationCheck::Rejected(IsolationFailure::PermissionMode));
        assert_eq!(
            rejected_exec.take_initial_event(),
            Some(AiEvent::IsolationChecked(IsolationCheck::Rejected(
                IsolationFailure::PermissionMode
            )))
        );
        assert_eq!(
            rejected_exec.decode_line(r#"{"type":"turn.started"}"#),
            Err(StreamDecodeError::Order(
                StreamOrderError::EventAfterRejection
            ))
        );
    }

    #[test]
    fn property_used_percent_exact_decimal_and_unknown_methods_roundtrip() {
        let mut runner = TestRunner::new(Config::with_cases(testkit::property_cases()));
        runner
            .run(&(0_u64..=100_u64, "[a-z]{3,10}"), |(percent, suffix)| {
                let rate_line = format!(
                    r#"{{"method":"account/rateLimits/updated","params":{{"ordinaryUsageAllowed":true,"rateLimits":{{"primary":{{"usedPercent":{percent}}}}}}}}}"#
                );
                let unknown_line = format!(r#"{{"method":"custom/{suffix}","params":{{}}}}"#);
                let events = decode_app_server_lines(
                    analysis_expectation(),
                    [
                        startup_thread_ok(),
                        startup_mcp_ok(),
                        rate_line.as_str(),
                        unknown_line.as_str(),
                    ],
                )
                .expect("decode property");
                let expected_util = match percent {
                    100_u64 => util("1.00"),
                    value if value < 10_u64 => util(&format!("0.0{value}")),
                    value => util(&format!("0.{value}")),
                };
                proptest::prop_assert_eq!(
                    events,
                    vec![
                        AiEvent::IsolationChecked(IsolationCheck::Passed),
                        AiEvent::Quota(AiUsage::Available(expected_util)),
                        AiEvent::Unknown(text(&unknown_line)),
                    ]
                );
                Ok(())
            })
            .expect("proptest");
    }

    #[test]
    fn exec_rejects_unclosed_tool_call_and_maps_usage_limit_error() {
        let unclosed = [
            r#"{"type":"item.started","item":{"id":"u1","type":"mcp_tool_call","server":"app","tool":"get_portfolio","arguments":{},"status":"in_progress"}}"#,
            r#"{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":2}}"#,
        ];
        assert_eq!(
            decode_exec_lines(IsolationCheck::Passed, unclosed),
            Err(StreamDecodeError::Order(StreamOrderError::UnclosedToolCall))
        );

        let limit = [
            r#"{"type":"turn.failed","error":{"code":"usage_limit_reached","resetsAt":1794394800}}"#,
        ];
        assert_eq!(
            decode_exec_lines(IsolationCheck::Passed, limit).expect("limit"),
            vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                AiEvent::RateLimited {
                    resets_at: ResetTime::Known(utc_seconds(1_794_394_800_i64)),
                },
            ]
        );
    }
}
