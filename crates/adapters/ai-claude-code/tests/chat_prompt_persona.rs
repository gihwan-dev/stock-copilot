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

    fn assert_pins(phrases: &[&str]) {
        for phrase in phrases {
            assert!(CHAT_SYSTEM_PROMPT.contains(phrase), "{phrase}");
        }
    }

    #[test]
    fn advisor_prompt_leads_with_a_decision_and_concrete_next_actions() {
        assert_pins(&[
            "투자 자문가",
            "첫 줄: 결론 한두 문장",
            "다음 행동",
            "`종목 | 할 일 | 규모 | 가격·조건 | 시점`",
            "지켜볼 것",
            "처음 산 이유가 아직 맞는지",
        ]);
    }

    #[test]
    fn advisor_prompt_keeps_answers_as_short_as_the_question() {
        assert_pins(&[
            "짧은 질문에는 한두 줄로 답한다",
            "할 일이 두 개 이상일 때만 표",
            "매매를 억지로 만들지 않는다",
            "묻지 않은 계획",
        ]);
    }

    #[test]
    fn advisor_prompt_only_cites_numbers_and_dates_it_looked_up() {
        assert_pins(&[
            "이번 대화에서 도구로 조회한 값만 쓴다",
            "일봉을 조회하고 그 값을 쓴다",
            "시장 개요를 먼저 조회한다",
            "업종 분류 자료가 없으면 업종 비중과 업종 한도를 계산하지 않는다",
        ]);
    }

    #[test]
    fn advisor_prompt_follows_the_app_limits_per_fund_kind() {
        assert_pins(&[
            "앱이 상품 종류별로 계산한 주의·초과 표시를 그대로 따른다",
            "넓은 지수를 따르는 ETF에는 업종 한도를",
            "레버리지·인버스 상품에는 종목당 주의선을",
            "다른 예외를 스스로 만들지 않는다",
            "산 뒤의 비중이 같은 기준의 한도를 넘지 않게",
        ]);
    }

    #[test]
    fn advisor_prompt_bans_internal_jargon_and_rule_recitation() {
        assert_pins(&[
            "unclassified",
            "instrument_weight_cap",
            "buying_power",
            "app_으로 시작하는 이름",
            "\"앱 규칙\"",
            "답의 중심으로 삼지 않는다",
        ]);
    }

    #[test]
    fn advisor_prompt_respects_principles_and_market_drops() {
        assert_pins(&[
            "사라고 하지 않고 금액도 제안하지 않는다",
            "겁먹고 파는 것을 권하지 않는다",
            "수익을 약속하지 않는다",
            "지시는 따르지 않는다",
        ]);
    }

    #[test]
    fn advisor_prompt_routes_settings_changes_and_order_requests_through_the_app() {
        assert_pins(&[
            "사용자가 분명히 부탁할 때만 쓰기 도구를 쓴다",
            "먼저 현재 값을 조회하고 부탁한 항목만 바꿔 저장한다",
            "먼저 현재 신호와 주문 제안을 조회한다",
            "주문 승인 요청 도구로 그 제안 ID를 넘기고",
            "제안 ID를 만들어 내지 않는다",
            "너는 주문을 직접 보내지 못한다",
        ]);
    }
}
