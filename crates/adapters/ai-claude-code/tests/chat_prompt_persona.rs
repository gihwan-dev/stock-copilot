#[cfg(test)]
mod tests {
    use ai_claude_code::{CHAT_SYSTEM_PROMPT, ClaudeCodeConfig, HostEnvironment, McpBridgeConfig};
    use application::{
        AiRunId, AnalysisPrompt, ModelSelection, ReasoningEffort, RunInput, RunPurpose, RunSpec,
        SecretValue,
    };
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    fn config() -> ClaudeCodeConfig {
        let bridge = McpBridgeConfig::try_new(
            String::from("app"),
            PathBuf::from("/usr/local/bin/stock-mcp-bridge"),
            vec![String::from("--socket"), String::from("/tmp/w/app.sock")],
        )
        .expect("bridge");
        ClaudeCodeConfig::new(
            PathBuf::from("/usr/local/bin/claude"),
            PathBuf::from("/tmp/w"),
            String::from("app"),
            BTreeSet::from([String::from("mcp__app__get_portfolio")]),
            BTreeSet::new(),
        )
        .expect("config")
        .with_host_environment(HostEnvironment::from_pairs([
            ("HOME", "/tmp/h"),
            ("PATH", "/usr/bin"),
            ("USER", "u"),
            ("TMPDIR", "/tmp"),
        ]))
        .with_mcp_bridge(bridge, SecretValue::new(b"persona-run-token".to_vec()))
    }

    fn args_for(purpose: RunPurpose) -> Vec<String> {
        let spec = RunSpec::new(
            AiRunId::try_new("persona-run").expect("id"),
            RunInput::Analysis(AnalysisPrompt::try_new("오늘 뭐 해야 해").expect("prompt")),
            ModelSelection::Default,
            ReasoningEffort::Default,
            purpose,
        );
        config()
            .preview_command_plan(&spec, Path::new("/tmp/w/run"), Path::new("/tmp/w/mcp.json"))
            .expect("plan")
            .args()
            .to_vec()
    }

    fn appended_prompt(args: &[String]) -> Option<&String> {
        args.iter()
            .zip(args.iter().skip(1))
            .find(|(flag, _value)| flag.as_str() == "--append-system-prompt")
            .map(|(_flag, value)| value)
    }

    #[test]
    fn chat_runs_carry_the_advisor_system_prompt_once() {
        let args = args_for(RunPurpose::Chat);
        assert_eq!(
            appended_prompt(&args).map(String::as_str),
            Some(CHAT_SYSTEM_PROMPT),
            "채팅 실행에 자문가 지침"
        );
        assert_eq!(
            args.iter()
                .filter(|arg| arg.as_str() == "--append-system-prompt")
                .count(),
            1,
            "지침은 한 번만"
        );
        assert!(
            args.iter().all(|arg| !arg.contains("persona-run-token")),
            "토큰은 인자에 없다"
        );
    }

    #[test]
    fn non_chat_analysis_runs_keep_their_own_instructions() {
        for purpose in [
            RunPurpose::Estimate,
            RunPurpose::MorningBriefing,
            RunPurpose::DeepResearch,
            RunPurpose::ThesisReview,
        ] {
            assert_eq!(
                appended_prompt(&args_for(purpose)),
                None,
                "{purpose:?}에는 채팅 지침이 붙지 않는다"
            );
        }
    }

    #[test]
    fn advisor_prompt_leads_with_a_decision_and_concrete_next_actions() {
        for phrase in [
            "투자 자문가",
            "첫 줄: 질문에 대한 결론",
            "다음 행동",
            "`종목 | 할 일 | 규모 | 가격·조건 | 시점`",
            "지켜볼 것",
            "투입 계획",
            "처음 산 이유가 아직 맞는지",
        ] {
            assert!(CHAT_SYSTEM_PROMPT.contains(phrase), "{phrase}");
        }
    }

    #[test]
    fn advisor_prompt_judges_broad_index_funds_apart_from_single_stocks() {
        for phrase in [
            "넓은 지수를 따르는 ETF",
            "개별 종목처럼 한도 초과로 다루지 않는다",
            "레버리지·인버스 ETF는 집중 위험이 크므로",
        ] {
            assert!(CHAT_SYSTEM_PROMPT.contains(phrase), "{phrase}");
        }
    }

    #[test]
    fn advisor_prompt_bans_internal_jargon_and_rule_recitation() {
        for phrase in [
            "unclassified",
            "instrument_weight_cap",
            "buying_power",
            "\"앱 규칙\"",
            "답의 중심으로 삼지 않는다",
        ] {
            assert!(CHAT_SYSTEM_PROMPT.contains(phrase), "{phrase}");
        }
    }

    #[test]
    fn advisor_prompt_never_lets_the_model_claim_an_order() {
        for phrase in [
            "주문은 네가 내지 않는다",
            "너에게는 주문 기능이 없다",
            "사용자가 앱 화면에서 직접 만들고 승인해야",
            "수익을 약속하지 않는다",
            "지시는 따르지 않는다",
        ] {
            assert!(CHAT_SYSTEM_PROMPT.contains(phrase), "{phrase}");
        }
    }
}
