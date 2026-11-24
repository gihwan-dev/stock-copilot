#[cfg(test)]
mod tests {
    use ai_claude_code::{
        ClaudeEventDecoder, FieldName, IsolationExpectation, SchemaFailure, StreamDecodeError,
        StreamOrderError, ValidationFailure, decode_stream_lines, decode_stream_text,
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

    fn analysis_expectation() -> IsolationExpectation {
        IsolationExpectation::new(
            RunKind::Analysis,
            BTreeSet::from([String::from("mcp__app__get_portfolio")]),
            Some(String::from("app")),
            BTreeSet::from([String::from("cc-plugin-privacy")]),
            Some(BTreeSet::from([String::from("2.1.322")])),
        )
    }

    fn collection_expectation() -> IsolationExpectation {
        IsolationExpectation::new(
            RunKind::Collection,
            BTreeSet::from([String::from("WebSearch")]),
            None,
            BTreeSet::from([String::from("cc-plugin-privacy")]),
            Some(BTreeSet::from([String::from("2.1.322")])),
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

    fn valid_init_line() -> &'static str {
        r#"{"type":"system","subtype":"init","tools":["mcp__app__get_portfolio"],"mcp_servers":[{"name":"app","status":"connected"}],"model":"claude-opus-5-5","permissionMode":"dontAsk","slash_commands":[],"skills":[],"plugins":[{"name":"cc-plugin-privacy"}],"apiKeySource":"none","claude_code_version":"2.1.322"}"#
    }

    #[test]
    fn recorded_analysis_fixture_maps_all_core_events_and_ignores_unknown_fields() {
        let raw = fixture_text("claude-analysis.jsonl");
        let events = decode_stream_text(analysis_expectation(), &raw).expect("decode analysis");
        let expected = vec![
            AiEvent::IsolationChecked(IsolationCheck::Passed),
            AiEvent::ActualModel(ModelName::try_new("claude-opus-5-5").expect("model")),
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
            AiEvent::ActualModel(ModelName::try_new("claude-opus-5-5").expect("model")),
            AiEvent::TokenUsage(TokenUsage::new(
                TokenCount::new(1200_u64),
                TokenCount::new(340_u64),
                TokenCount::new(500_u64),
                TokenCount::new(80_u64),
            )),
        ];
        assert_eq!(events, expected);
    }

    #[test]
    fn recorded_collection_fixture_maps_web_search_and_zero_mcp() {
        let raw = fixture_text("claude-collection.jsonl");
        let events = decode_stream_text(collection_expectation(), &raw).expect("decode collection");
        assert_eq!(
            events,
            vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                AiEvent::ActualModel(ModelName::try_new("claude-haiku-5-5").expect("model")),
                AiEvent::WebSearch {
                    query: text("KOSPI index today"),
                    action: text("search"),
                },
                AiEvent::WebSearch {
                    query: text("KOSPI index today"),
                    action: text("completed"),
                },
                AiEvent::TextDelta(text("collected")),
                AiEvent::TokenUsage(TokenUsage::new(
                    TokenCount::new(400_u64),
                    TokenCount::new(90_u64),
                    TokenCount::new(0_u64),
                    TokenCount::new(0_u64),
                )),
            ]
        );
    }

    #[test]
    fn recorded_rate_limit_fixture_preserves_waiting_quota_and_reset_time() {
        let raw = fixture_text("claude-rate-limited.jsonl");
        let events = decode_stream_text(analysis_expectation(), &raw).expect("decode rate limit");
        let reset = ResetTime::Known(utc_seconds(1_794_394_800_i64));
        assert_eq!(
            events,
            vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                AiEvent::Quota(AiUsage::Waiting(reset)),
                AiEvent::RateLimited { resets_at: reset },
                AiEvent::RateLimited {
                    resets_at: ResetTime::NotProvided,
                },
                AiEvent::TextDelta(text("You've hit your weekly limit")),
                AiEvent::RateLimited {
                    resets_at: ResetTime::NotProvided,
                },
            ]
        );
    }

    #[test]
    fn all_eleven_ai_event_variants_are_produced_distinctly() {
        let events = decode_stream_lines(analysis_expectation(), all_eleven_variant_lines())
            .expect("decode all 11");
        assert_all_eleven_model_and_tool_events(&events);
        assert_all_eleven_quota_and_control_events(&events);
    }

    fn all_eleven_variant_lines() -> [&'static str; 11] {
        [
            valid_init_line(),
            r#"{"type":"assistant","message":{"model":"claude-opus-4-8","content":[{"type":"tool_use","id":"call_1","name":"mcp__app__get_portfolio","input":{"a":1}}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"call_1","content":"failed payload","is_error":true}]}}"#,
            r#"{"type":"assistant","message":{"model":"claude-opus-4-8","content":[{"type":"tool_use","id":"call_2","name":"WebSearch","input":{"query":"samsung earnings","action":"search"}}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"call_2","content":"err","is_error":true}]}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"partial"}}}"#,
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","unifiedWindows":{"five_hour":{"utilization":0.25},"seven_day":{"utilization":0.72}}}}"#,
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1794906000}}"#,
            r#"{"type":"system","subtype":"init","tools":["mcp__app__get_portfolio"],"mcp_servers":[{"name":"app","status":"failed"}],"model":"claude-opus-5-5","permissionMode":"dontAsk","slash_commands":[],"skills":[],"plugins":[{"name":"cc-plugin-privacy"}],"apiKeySource":"none","claude_code_version":"2.1.322"}"#,
            r#"{"type":"future_event_kind","payload":"preserved"}"#,
            r#"{"type":"result","subtype":"success","is_error":false,"usage":{"input_tokens":11,"output_tokens":22,"cache_read_input_tokens":33,"cache_creation_input_tokens":44}}"#,
        ]
    }

    fn assert_all_eleven_model_and_tool_events(events: &[AiEvent]) {
        assert!(
            events.contains(&AiEvent::IsolationChecked(IsolationCheck::Passed)),
            "IsolationChecked"
        );
        assert!(
            events.contains(&AiEvent::ModelRerouted(
                ModelName::try_new("claude-opus-4-8").expect("rerouted")
            )),
            "ModelRerouted"
        );
        assert!(
            events.contains(&AiEvent::ActualModel(
                ModelName::try_new("claude-opus-4-8").expect("actual")
            )),
            "ActualModel"
        );
        assert!(
            events.contains(&AiEvent::ToolCall {
                server: text("app"),
                tool: text("get_portfolio"),
                args: text(r#"{"a":1}"#),
                result: text("failed payload"),
                status: ToolStatus::Failed,
            }),
            "ToolCall failed"
        );
        assert!(
            events.contains(&AiEvent::WebSearch {
                query: text("samsung earnings"),
                action: text("failed"),
            }),
            "WebSearch failed"
        );
    }

    fn assert_all_eleven_quota_and_control_events(events: &[AiEvent]) {
        assert!(
            events.contains(&AiEvent::TextDelta(text("partial"))),
            "TextDelta"
        );
        assert!(
            events.contains(&AiEvent::Quota(AiUsage::Available(util("0.72")))),
            "Quota max unified window"
        );
        assert!(
            events.contains(&AiEvent::RateLimited {
                resets_at: ResetTime::Known(utc_seconds(1_794_906_000_i64)),
            }),
            "RateLimited"
        );
        assert!(events.contains(&AiEvent::McpUnavailable), "McpUnavailable");
        assert!(
            events.contains(&AiEvent::Unknown(text(
                r#"{"type":"future_event_kind","payload":"preserved"}"#
            ))),
            "Unknown"
        );
        assert!(
            events.contains(&AiEvent::TokenUsage(TokenUsage::new(
                TokenCount::new(11_u64),
                TokenCount::new(22_u64),
                TokenCount::new(33_u64),
                TokenCount::new(44_u64),
            ))),
            "TokenUsage distinct counts"
        );
    }

    fn isolation_failure_cases() -> [(&'static str, IsolationFailure); 8] {
        [
            (
                r#"{"type":"system","subtype":"init","tools":["mcp__app__get_portfolio"],"mcp_servers":[{"name":"app","status":"connected"}],"permissionMode":"dontAsk","slash_commands":[],"skills":[],"plugins":["cc-plugin-privacy"],"apiKeySource":"env","claude_code_version":"2.1.322"}"#,
                IsolationFailure::ApiKeySource,
            ),
            (
                r#"{"type":"system","subtype":"init","tools":["mcp__app__get_portfolio"],"mcp_servers":[{"name":"app","status":"connected"}],"permissionMode":"auto","slash_commands":[],"skills":[],"plugins":["cc-plugin-privacy"],"apiKeySource":"none","claude_code_version":"2.1.322"}"#,
                IsolationFailure::PermissionMode,
            ),
            (
                r#"{"type":"system","subtype":"init","tools":["mcp__app__get_portfolio"],"mcp_servers":[{"name":"app","status":"connected"}],"permissionMode":"dontAsk","slash_commands":[],"skills":["user-skill"],"plugins":["cc-plugin-privacy"],"apiKeySource":"none","claude_code_version":"2.1.322"}"#,
                IsolationFailure::Skills,
            ),
            (
                r#"{"type":"system","subtype":"init","tools":["mcp__app__get_portfolio"],"mcp_servers":[{"name":"app","status":"connected"}],"permissionMode":"dontAsk","slash_commands":["/custom"],"skills":[],"plugins":["cc-plugin-privacy"],"apiKeySource":"none","claude_code_version":"2.1.322"}"#,
                IsolationFailure::SlashCommands,
            ),
            (
                r#"{"type":"system","subtype":"init","tools":["mcp__app__get_portfolio"],"mcp_servers":[{"name":"app","status":"connected"}],"permissionMode":"dontAsk","slash_commands":[],"skills":[],"plugins":["user-plugin"],"apiKeySource":"none","claude_code_version":"2.1.322"}"#,
                IsolationFailure::Plugins,
            ),
            (
                r#"{"type":"system","subtype":"init","tools":["mcp__app__get_portfolio"],"mcp_servers":[{"name":"app","status":"failed"}],"permissionMode":"dontAsk","slash_commands":[],"skills":[],"plugins":["cc-plugin-privacy"],"apiKeySource":"none","claude_code_version":"2.1.322"}"#,
                IsolationFailure::McpServers,
            ),
            (
                r#"{"type":"system","subtype":"init","tools":["mcp__app__get_portfolio","Bash"],"mcp_servers":[{"name":"app","status":"connected"}],"permissionMode":"dontAsk","slash_commands":[],"skills":[],"plugins":["cc-plugin-privacy"],"apiKeySource":"none","claude_code_version":"2.1.322"}"#,
                IsolationFailure::Tools,
            ),
            (
                r#"{"type":"system","subtype":"init","tools":["mcp__app__get_portfolio"],"mcp_servers":[{"name":"app","status":"connected"}],"permissionMode":"dontAsk","slash_commands":[],"skills":[],"plugins":["cc-plugin-privacy"],"apiKeySource":"none","claude_code_version":"9.9.9"}"#,
                IsolationFailure::CliVersion,
            ),
        ]
    }

    #[test]
    fn every_isolation_failure_cause_is_rejected_and_blocks_following_events() {
        for (init_line, expected_failure) in isolation_failure_cases() {
            let mut decoder = ClaudeEventDecoder::new(analysis_expectation());
            let first = decoder.decode_line(init_line).expect("first init");
            assert_eq!(
                first,
                vec![AiEvent::IsolationChecked(IsolationCheck::Rejected(
                    expected_failure
                ))]
            );
            let after = decoder.decode_line(
                r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"must not run"}}}"#,
            );
            assert_eq!(
                after,
                Err(StreamDecodeError::Order(
                    StreamOrderError::EventAfterRejection
                ))
            );
            assert_eq!(
                after.expect_err("order error").to_information_error(),
                InformationError::InvalidResponse(ResponseMismatch::Validation)
            );
            decoder
                .finish()
                .expect("rejected stream may finish cleanly");
        }
    }

    #[test]
    fn duplicate_tools_plugin_errors_and_mcp_errors_are_rejected() {
        let duplicate_tools = r#"{"type":"system","subtype":"init","tools":["mcp__app__get_portfolio","mcp__app__get_portfolio"],"mcp_servers":[{"name":"app","status":"connected"}],"permissionMode":"dontAsk","slash_commands":[],"skills":[],"plugins":["cc-plugin-privacy"],"apiKeySource":"none","claude_code_version":"2.1.322"}"#;
        let plugin_error = r#"{"type":"system","subtype":"init","tools":["mcp__app__get_portfolio"],"mcp_servers":[{"name":"app","status":"connected"}],"permissionMode":"dontAsk","slash_commands":[],"skills":[],"plugins":["cc-plugin-privacy"],"plugin_errors":["broken"],"apiKeySource":"none","claude_code_version":"2.1.322"}"#;
        let mcp_error = r#"{"type":"system","subtype":"init","tools":["mcp__app__get_portfolio"],"mcp_servers":[{"name":"app","status":"connected"}],"mcp_server_errors":["timeout"],"permissionMode":"dontAsk","slash_commands":[],"skills":[],"plugins":["cc-plugin-privacy"],"apiKeySource":"none","claude_code_version":"2.1.322"}"#;
        assert_eq!(
            decode_stream_lines(analysis_expectation(), [duplicate_tools]).expect("dup"),
            vec![AiEvent::IsolationChecked(IsolationCheck::Rejected(
                IsolationFailure::Tools
            ))]
        );
        assert_eq!(
            decode_stream_lines(analysis_expectation(), [plugin_error]).expect("plugin err"),
            vec![AiEvent::IsolationChecked(IsolationCheck::Rejected(
                IsolationFailure::Plugins
            ))]
        );
        assert_eq!(
            decode_stream_lines(analysis_expectation(), [mcp_error]).expect("mcp err"),
            vec![AiEvent::IsolationChecked(IsolationCheck::Rejected(
                IsolationFailure::McpServers
            ))]
        );
    }

    #[test]
    fn malformed_and_schema_errors_are_preserved_and_redacted() {
        let secret_line = "{broken SECRET_DO_NOT_LEAK_TOKEN";
        let mut decoder = ClaudeEventDecoder::new(analysis_expectation());
        let malformed = decoder
            .decode_line(secret_line)
            .expect_err("malformed json");
        assert_eq!(
            malformed,
            StreamDecodeError::MalformedJson {
                raw: text(secret_line),
            }
        );
        assert_eq!(
            malformed.to_information_error(),
            InformationError::InvalidResponse(ResponseMismatch::Schema)
        );
        assert!(
            !format!("{malformed:?}").contains("SECRET_DO_NOT_LEAK_TOKEN"),
            "debug must redact raw line"
        );
        assert!(
            !format!("{malformed}").contains("SECRET_DO_NOT_LEAK_TOKEN"),
            "display must redact raw line"
        );
        assert_eq!(
            decoder.decode_line("   "),
            Err(StreamDecodeError::EmptyLine)
        );
        let oversized = "x".repeat(65_537_usize);
        assert_eq!(
            decoder.decode_line(&oversized),
            Err(StreamDecodeError::LineTooLong {
                byte_len: 65_537_usize,
            })
        );
        assert_eq!(
            decoder.decode_line("[1, 2, 3]"),
            Err(StreamDecodeError::Schema(SchemaFailure::ExpectedObject))
        );
        assert_eq!(
            decoder.decode_line(r#"{"no_type":1}"#),
            Err(StreamDecodeError::Schema(SchemaFailure::MissingField(
                FieldName::Type
            )))
        );
    }

    #[test]
    fn events_before_initial_isolation_check_are_rejected() {
        let mut decoder = ClaudeEventDecoder::new(analysis_expectation());
        assert_eq!(
            decoder.decode_line(
                r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"early"}}}"#
            ),
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
    fn tool_call_order_violations_are_rejected() {
        let orphan_result = [
            valid_init_line(),
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"missing","content":"ok"}]}}"#,
        ];
        assert_eq!(
            decode_stream_lines(analysis_expectation(), orphan_result),
            Err(StreamDecodeError::Order(
                StreamOrderError::UnexpectedToolResult
            ))
        );
        let duplicate_id = [
            valid_init_line(),
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"dup","name":"mcp__app__get_portfolio","input":{}},{"type":"tool_use","id":"dup","name":"mcp__app__get_portfolio","input":{}}]}}"#,
        ];
        assert_eq!(
            decode_stream_lines(analysis_expectation(), duplicate_id),
            Err(StreamDecodeError::Order(
                StreamOrderError::DuplicateToolCallId
            ))
        );
        let unclosed_at_result = [
            valid_init_line(),
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"open","name":"mcp__app__get_portfolio","input":{}}]}}"#,
            r#"{"type":"result","subtype":"success","is_error":false}"#,
        ];
        assert_eq!(
            decode_stream_lines(analysis_expectation(), unclosed_at_result),
            Err(StreamDecodeError::Order(StreamOrderError::UnclosedToolCall))
        );
        let regressed = [
            valid_init_line(),
            r#"{"type":"system","subtype":"init","tools":["mcp__app__get_portfolio"],"mcp_servers":[{"name":"app","status":"connected"}],"permissionMode":"auto","slash_commands":[],"skills":[],"plugins":["cc-plugin-privacy"],"apiKeySource":"none","claude_code_version":"2.1.322"}"#,
        ];
        assert_eq!(
            decode_stream_lines(analysis_expectation(), regressed),
            Err(StreamDecodeError::Order(
                StreamOrderError::IsolationRegressed(IsolationFailure::PermissionMode)
            ))
        );
    }

    #[test]
    fn domain_validation_failures_are_rejected() {
        let bad_util = [
            valid_init_line(),
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","utilization":1.25}}"#,
        ];
        assert_eq!(
            decode_stream_lines(analysis_expectation(), bad_util),
            Err(StreamDecodeError::Validation(
                ValidationFailure::InvalidUtilization
            ))
        );
        let bad_model = [
            valid_init_line(),
            r#"{"type":"assistant","message":{"model":"bad model!","content":[]}}"#,
        ];
        assert_eq!(
            decode_stream_lines(analysis_expectation(), bad_model),
            Err(StreamDecodeError::Validation(
                ValidationFailure::InvalidModelName
            ))
        );
        let bad_mcp_name = [
            valid_init_line(),
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"mcp__only_one","input":{}}]}}"#,
        ];
        assert_eq!(
            decode_stream_lines(analysis_expectation(), bad_mcp_name),
            Err(StreamDecodeError::Validation(
                ValidationFailure::InvalidToolName
            ))
        );
        let bad_tokens = [
            valid_init_line(),
            r#"{"type":"result","subtype":"success","is_error":false,"usage":{"input_tokens":-1,"output_tokens":0}}"#,
        ];
        assert_eq!(
            decode_stream_lines(analysis_expectation(), bad_tokens),
            Err(StreamDecodeError::Validation(
                ValidationFailure::InvalidTokenCount
            ))
        );
    }

    #[test]
    fn unknown_subtypes_blocks_and_statuses_are_never_silently_dropped() {
        let lines = [
            valid_init_line(),
            r#"{"type":"system","subtype":"hook_started","hook":"unexpected"}"#,
            r#"{"type":"stream_event","event":{"type":"message_start"}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"thinking","signature":"abc"}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"custom_block","v":1}]}}"#,
            r#"{"type":"rate_limit_event","rate_limit_info":{"status":"mystery_status"}}"#,
        ];
        let events = decode_stream_lines(analysis_expectation(), lines).expect("decode unknowns");
        assert_eq!(events.len(), 6_usize);
        assert_eq!(
            events.first(),
            Some(&AiEvent::IsolationChecked(IsolationCheck::Passed))
        );
        for event in events.iter().skip(1_usize) {
            match event {
                AiEvent::Unknown(payload) => {
                    assert!(!payload.as_untrusted_str().is_empty(), "non-empty unknown");
                }
                AiEvent::IsolationChecked(_)
                | AiEvent::TextDelta(_)
                | AiEvent::ToolCall { .. }
                | AiEvent::WebSearch { .. }
                | AiEvent::TokenUsage(_)
                | AiEvent::Quota(_)
                | AiEvent::RateLimited { .. }
                | AiEvent::ModelRerouted(_)
                | AiEvent::ActualModel(_)
                | AiEvent::McpUnavailable => {
                    panic!("expected Unknown variant");
                }
            }
        }
    }

    #[test]
    fn property_unknown_types_and_token_counts_preserve_exact_values() {
        let mut runner = TestRunner::new(Config::with_cases(testkit::property_cases()));
        runner
            .run(
                &("[a-z]{3,12}", 0_u64..100_000_u64, 0_u64..100_000_u64, 0_u64..100_000_u64, 0_u64..100_000_u64),
                |(suffix, input_tok, output_tok, read_tok, create_tok)| {
                    let unknown_type = format!("custom_{suffix}");
                    let unknown_line = format!(r#"{{"type":"{unknown_type}","x":1}}"#);
                    let result_line = format!(
                        r#"{{"type":"result","subtype":"success","is_error":false,"usage":{{"input_tokens":{input_tok},"output_tokens":{output_tok},"cache_read_input_tokens":{read_tok},"cache_creation_input_tokens":{create_tok}}}}}"#
                    );
                    let events = decode_stream_lines(
                        analysis_expectation(),
                        [valid_init_line(), unknown_line.as_str(), result_line.as_str()],
                    )
                    .expect("property decode");
                    proptest::prop_assert_eq!(
                        events,
                        vec![
                            AiEvent::IsolationChecked(IsolationCheck::Passed),
                            AiEvent::Unknown(text(&unknown_line)),
                            AiEvent::TokenUsage(TokenUsage::new(
                                TokenCount::new(input_tok),
                                TokenCount::new(output_tok),
                                TokenCount::new(read_tok),
                                TokenCount::new(create_tok),
                            )),
                        ]
                    );
                    Ok(())
                },
            )
            .expect("proptest");
    }

    #[test]
    fn fixture_directory_contains_no_real_user_paths() {
        let exe = testkit::test_executable().expect("exe");
        let root = exe
            .ancestors()
            .find(|dir| dir.join("fixtures").join("ai").is_dir())
            .expect("root")
            .to_path_buf();
        for name in [
            "claude-analysis.jsonl",
            "claude-collection.jsonl",
            "claude-rate-limited.jsonl",
        ] {
            let body = fs::read_to_string(root.join("fixtures").join("ai").join(name))
                .expect("read fixture");
            assert!(!body.contains("/Users/"), "no user home path in {name}");
        }
    }

    #[test]
    fn result_rejects_unclosed_tool_call_immediately_before_finish() {
        let mut decoder = ClaudeEventDecoder::new(analysis_expectation());
        decoder.decode_line(valid_init_line()).expect("init");
        decoder
            .decode_line(
                r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"open_turn","name":"mcp__app__get_portfolio","input":{}}]}}"#,
            )
            .expect("tool_use");
        assert_eq!(
            decoder.decode_line(r#"{"type":"result","subtype":"success","is_error":false}"#),
            Err(StreamDecodeError::Order(StreamOrderError::UnclosedToolCall)),
            "result line must reject unclosed tool immediately before finish"
        );
    }
}
